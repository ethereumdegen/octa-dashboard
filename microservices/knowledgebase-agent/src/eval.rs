//! Spice eval harness for the RAG chat flow.
//!
//! Drives the real [`RagAgent`] against a live knowledge base and asserts its
//! behaviour with Spice (a Rust eval framework for nondeterministic LLM agents,
//! <https://github.com/ethereumdegen/spice>).
//!
//! This is an on-demand *integration* eval: it needs a live OpenAI key, Postgres,
//! and a seeded knowledge base, so it is `#[ignore]`d and never runs in a plain
//! `cargo test`. Run it explicitly:
//!
//! ```bash
//! OPENAI_API_KEY=sk-... \
//! DATABASE_URL=postgres://... \
//! KB_EVAL_ID=<uuid-of-a-seeded-kb> \
//!   cargo test -p knowledgebase-agent -- --ignored rag_chat_eval --nocapture
//! ```
//!
//! Spice asserts on tool usage and output text. It does not inspect HTTP wire
//! formats, so it would not *directly* see a serializer bug like the rig-core
//! `input_text` 400 — but that failure surfaces through [`RagAgent`] as a
//! `RunOutcome::Failed` collapsed into a `"Query failed in '<node>': ..."`
//! answer, which the adapter lifts into `AgentOutput.error`. The multi-turn
//! case below therefore guards exactly that class of regression.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use spice_framework::{
    suite, test, AgentConfig, AgentOutput, AgentUnderTest, Runner, RunnerConfig, SpiceError,
    ToolCall, Turn,
};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::models::chat_session::{ChatMessage, ChatRole};
use crate::services::rag_agent::RagAgent;

/// The KB-scoped tools the RAG agent is allowed to call. Mirrors the registry in
/// [`crate::services::rag_agent::RagAgent::new_for_kb`]; used for allowlist asserts.
const ALLOWED_TOOLS: &[&str] = &[
    "list_documents",
    "search_index",
    "navigate_tree",
    "search_pages",
    "read_page",
];

/// Spice adapter wrapping the production [`RagAgent`].
struct KbAgentUnderTest {
    agent: RagAgent,
}

#[async_trait]
impl AgentUnderTest for KbAgentUnderTest {
    async fn run(
        &self,
        user_message: &str,
        config: &AgentConfig,
    ) -> Result<AgentOutput, SpiceError> {
        // Spice has no first-class multi-turn input, so prior turns ride in via
        // `config.data["history"]` and the adapter decodes them.
        let history = decode_history(&config.data);

        let resp = self
            .agent
            .query_with_history(user_message, &history)
            .await
            .map_err(|e| SpiceError::AgentError(e.to_string()))?;

        // `RagAgent::extract_response` collapses graph failures (e.g. an OpenAI
        // 400) into the answer text rather than an `Err`. Lift that back into a
        // real error so `expect_no_error` / `expect_text_not_contains` bite.
        let error = (resp.answer.starts_with("Query failed")
            || resp.answer.starts_with("Query interrupted"))
        .then(|| resp.answer.clone());

        // The RAG agent reports tool *names* but not per-turn boundaries, so we
        // emit a single synthetic turn carrying the calls. That powers
        // expect_tools / expect_any_tool / allowlist checks; fine-grained
        // turn-range assertions are intentionally out of scope here.
        let tool_calls: Vec<ToolCall> = resp
            .tools_used
            .iter()
            .enumerate()
            .map(|(i, name)| ToolCall {
                id: format!("call_{i}"),
                name: name.clone(),
                arguments: serde_json::Value::Null,
            })
            .collect();

        let turn = Turn {
            index: 0,
            output_text: Some(resp.answer.clone()),
            tool_calls,
            tool_results: vec![],
            stop_reason: None,
            duration: Duration::ZERO,
        };

        Ok(AgentOutput {
            final_text: resp.answer,
            turns: vec![turn],
            tools_called: resp.tools_used,
            duration: Duration::ZERO,
            error,
        })
    }

    fn available_tools(&self, _config: &AgentConfig) -> Vec<String> {
        ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect()
    }

    fn name(&self) -> &str {
        "knowledgebase-rag-agent"
    }
}

/// Decode `config.data["history"]` — a list of `{ "role", "content" }` objects —
/// into the [`ChatMessage`]s `query_with_history` expects. Ids/timestamps are
/// synthetic; only `role` and `content` matter to the agent.
fn decode_history(data: &serde_json::Value) -> Vec<ChatMessage> {
    let Some(items) = data.get("history").and_then(|v| v.as_array()) else {
        return vec![];
    };
    items
        .iter()
        .filter_map(|m| {
            let content = m.get("content")?.as_str()?.to_string();
            let role = match m.get("role").and_then(|r| r.as_str()) {
                Some("assistant") => ChatRole::Assistant,
                _ => ChatRole::User,
            };
            Some(ChatMessage {
                id: Uuid::new_v4(),
                session_id: Uuid::nil(),
                role,
                content,
                metadata: None,
                created_at: chrono::Utc::now(),
            })
        })
        .collect()
}

#[tokio::test]
#[ignore = "live eval: requires OPENAI_API_KEY, DATABASE_URL, KB_EVAL_ID"]
async fn rag_chat_eval() {
    let _ = dotenvy::dotenv();
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let kb_id: Uuid = std::env::var("KB_EVAL_ID")
        .expect("KB_EVAL_ID (uuid of a seeded knowledge base) must be set")
        .parse()
        .expect("KB_EVAL_ID must be a valid uuid");

    let db = PgPoolOptions::new()
        .max_connections(4)
        .connect(&db_url)
        .await
        .expect("connect to database");

    let kb = crate::db::knowledgebases::get_by_id(&db, kb_id)
        .await
        .expect("load knowledge base")
        .expect("KB_EVAL_ID does not match any knowledge base");

    let agent = RagAgent::new_for_kb(&api_key, &kb, db).expect("build RAG agent");
    let agent: Arc<dyn AgentUnderTest> = Arc::new(KbAgentUnderTest { agent });

    let suite = suite(
        "knowledgebase RAG chat",
        vec![
            // Single-turn: the agent should retrieve and answer without error,
            // touching only its own tools.
            test("single-turn-smoke", "What topics does this knowledge base cover?")
                .name("Single-turn retrieval stays within allowlist and errors out cleanly")
                .expect_no_error()
                .expect_any_tool()
                .expect_tools_within_allowlist()
                .retries(2)
                .build(),
            // Multi-turn: a follow-up after a prior assistant turn. This is the
            // exact shape that produced the OpenAI Responses API 400
            // ("Invalid value: 'input_text'") before metalcraft 0.8.1. The 400
            // would surface as a "Query failed ..." answer -> AgentOutput.error.
            test("multi-turn-followup", "Can you go into more detail on that?")
                .name("Follow-up with prior assistant turn must not 400")
                .config_json(serde_json::json!({
                    "history": [
                        { "role": "user", "content": "Give me a one-line overview." },
                        { "role": "assistant", "content": "It's a knowledge base of indexed documents you can ask questions about." }
                    ]
                }))
                .expect_no_error()
                .expect_text_not_contains("Query failed")
                .expect_tools_within_allowlist()
                .retries(2)
                .build(),
        ],
    );

    let runner = Runner::new(RunnerConfig {
        concurrency: 1,
        ..Default::default()
    });
    let report = runner.run(suite, agent).await;
    report.print_console();

    assert_eq!(report.failed, 0, "{} eval test(s) failed", report.failed);
}
