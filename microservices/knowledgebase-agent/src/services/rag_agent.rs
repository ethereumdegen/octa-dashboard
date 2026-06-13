use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use metalcraft::{
    create_react_agent, AgentMessage, AgentState, CompiledGraph, Executor, GraphError,
    Result as McResult, RunOutcome, Tool, ToolRegistry,
};
use rig::client::CompletionClient;
use rig::providers::openai;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::models::chat_session::{ChatMessage, ChatRole};
use crate::models::knowledgebase::Knowledgebase;
use crate::services::llm::{LlmClient, LlmContext};

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

// ---------------------------------------------------------------------------
// KB-scoped tools
// ---------------------------------------------------------------------------

struct ListDocumentsTool {
    db: PgPool,
    kb_id: Uuid,
}

#[async_trait]
impl Tool for ListDocumentsTool {
    fn name(&self) -> &str {
        "list_documents"
    }
    fn description(&self) -> &str {
        "List all indexed documents in this knowledge base. Returns document IDs, filenames, page counts, and status."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
    async fn call(&self, _args: serde_json::Value) -> McResult<serde_json::Value> {
        let docs = db::documents::list_for_kb(&self.db, self.kb_id)
            .await
            .map_err(|e| GraphError::Node {
                node: "list_documents".into(),
                message: e.to_string(),
            })?;

        let results: Vec<serde_json::Value> = docs
            .iter()
            .filter(|d| matches!(d.status, crate::models::document::DocStatus::Indexed))
            .map(|d| {
                json!({
                    "id": d.id.to_string(),
                    "filename": d.filename,
                    "page_count": d.page_count,
                })
            })
            .collect();

        Ok(json!({ "documents": results, "count": results.len() }))
    }
}

struct SearchIndexTool {
    db: PgPool,
    kb_id: Uuid,
}

#[async_trait]
impl Tool for SearchIndexTool {
    fn name(&self) -> &str {
        "search_index"
    }
    fn description(&self) -> &str {
        "Search for relevant pages across indexed documents. Returns matching entities and page_map sections scored by keyword relevance. Use specific terms, acronyms, and keywords for best results. Optionally scope to a single document_id."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query or topic to find"
                },
                "document_id": {
                    "type": "string",
                    "description": "Optional: specific document UUID to search. If omitted, searches all documents in this KB."
                }
            },
            "required": ["query"]
        })
    }
    async fn call(&self, args: serde_json::Value) -> McResult<serde_json::Value> {
        let query = args["query"].as_str().unwrap_or("");
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let doc_id = args["document_id"].as_str();

        let indexes = if let Some(id_str) = doc_id {
            let id = Uuid::parse_str(id_str).map_err(|e| GraphError::Node {
                node: "search_index".into(),
                message: format!("invalid document_id: {e}"),
            })?;
            match db::page_indexes::get_document_index(&self.db, id)
                .await
                .map_err(|e| GraphError::Node {
                    node: "search_index".into(),
                    message: e.to_string(),
                })? {
                Some(idx) => vec![idx],
                None => vec![],
            }
        } else {
            db::page_indexes::get_document_indexes_for_kb(&self.db, self.kb_id)
                .await
                .map_err(|e| GraphError::Node {
                    node: "search_index".into(),
                    message: e.to_string(),
                })?
        };

        let mut all_hits: Vec<serde_json::Value> = Vec::new();

        for idx in &indexes {
            let doc_id_str = idx.document_id.to_string();

            // Score entity_index matches (exact entity name matching)
            let mut entity_hits: Vec<serde_json::Value> = Vec::new();
            if let Some(entity_index) = idx.root_index.get("entity_index").and_then(|v| v.as_object()) {
                for (entity_name, pages) in entity_index {
                    let entity_lower = entity_name.to_lowercase();
                    // Match if any query word matches entity name or entity contains query term
                    let matched = query_words.iter().any(|w| {
                        entity_lower.contains(w) || w.contains(entity_lower.as_str())
                    });
                    if matched {
                        entity_hits.push(json!({
                            "entity": entity_name,
                            "pages": pages,
                        }));
                    }
                }
            }

            // Score page_map entries by keyword overlap
            let mut page_map_hits: Vec<serde_json::Value> = Vec::new();
            if let Some(page_map) = idx.root_index.get("page_map").and_then(|v| v.as_array()) {
                for entry in page_map {
                    let theme = entry.get("theme").and_then(|v| v.as_str()).unwrap_or("");
                    let keywords: Vec<&str> = entry
                        .get("relevance_keywords")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|k| k.as_str()).collect())
                        .unwrap_or_default();

                    let theme_lower = theme.to_lowercase();
                    let keywords_lower: String = keywords.join(" ").to_lowercase();
                    let searchable = format!("{} {}", theme_lower, keywords_lower);

                    let score: usize = query_words
                        .iter()
                        .filter(|w| searchable.contains(*w))
                        .count();

                    if score > 0 {
                        page_map_hits.push(json!({
                            "theme": theme,
                            "pages": entry.get("pages"),
                            "relevance_keywords": keywords,
                            "score": score,
                        }));
                    }
                }
            }

            // Sort page_map hits by score descending
            page_map_hits.sort_by(|a, b| {
                let sa = a["score"].as_u64().unwrap_or(0);
                let sb = b["score"].as_u64().unwrap_or(0);
                sb.cmp(&sa)
            });

            if !entity_hits.is_empty() || !page_map_hits.is_empty() {
                let summary = idx.root_index.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                all_hits.push(json!({
                    "document_id": doc_id_str,
                    "document_summary": summary,
                    "entity_matches": entity_hits,
                    "page_map_matches": page_map_hits,
                }));
            }
        }

        // If no filtered hits, return summary-level info so agent knows what's available
        if all_hits.is_empty() {
            let summaries: Vec<serde_json::Value> = indexes
                .iter()
                .map(|idx| json!({
                    "document_id": idx.document_id.to_string(),
                    "summary": idx.root_index.get("summary").and_then(|v| v.as_str()).unwrap_or(""),
                    "key_themes": idx.root_index.get("key_themes").cloned().unwrap_or(json!([])),
                }))
                .collect();
            return Ok(json!({
                "matches": [],
                "no_direct_matches": true,
                "hint": "No entity or keyword matches found. Try broader terms, or use read_page to scan specific pages. Here are the available documents:",
                "available_documents": summaries,
            }));
        }

        Ok(json!({ "matches": all_hits }))
    }
}

// ---------------------------------------------------------------------------
// Navigate Tree: LLM reasons over document tree structure (PageIndex pattern)
// ---------------------------------------------------------------------------

struct NavigateTreeTool {
    db: PgPool,
    kb_id: Uuid,
    llm: Arc<LlmClient>,
}

const NAVIGATE_TREE_SYSTEM: &str = r#"You are a document navigation system. Given a query and a document's hierarchical tree structure (summaries only, no full text), identify which pages are most likely to contain the answer.

Reason step by step through the tree structure. Consider topic names, summaries, key entities, and relationships.

Reply in JSON format:
{
  "thinking": "<your reasoning about which sections are relevant and why>",
  "page_list": [1, 3, 5]
}

Rules:
- Output ONLY valid JSON, no markdown fences
- Return pages in order of relevance (most relevant first)
- Return at most 8 pages
- If no pages seem relevant, return an empty page_list and explain in thinking"#;

#[async_trait]
impl Tool for NavigateTreeTool {
    fn name(&self) -> &str {
        "navigate_tree"
    }
    fn description(&self) -> &str {
        "LLM-powered navigation of a document's tree structure. Reasons over page summaries, topics, and entities to find relevant sections — better than keyword search for complex or semantic queries. Requires a document_id."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The question or topic to find in the document"
                },
                "document_id": {
                    "type": "string",
                    "description": "The document UUID to navigate"
                }
            },
            "required": ["query", "document_id"]
        })
    }
    async fn call(&self, args: serde_json::Value) -> McResult<serde_json::Value> {
        let query = args["query"].as_str().unwrap_or("");
        let doc_id_str = args["document_id"]
            .as_str()
            .ok_or_else(|| GraphError::Node {
                node: "navigate_tree".into(),
                message: "document_id required".into(),
            })?;
        let doc_id = Uuid::parse_str(doc_id_str).map_err(|e| GraphError::Node {
            node: "navigate_tree".into(),
            message: format!("invalid document_id: {e}"),
        })?;

        // Fetch root index for document-level context
        let root_idx = db::page_indexes::get_document_index(&self.db, doc_id)
            .await
            .map_err(|e| GraphError::Node {
                node: "navigate_tree".into(),
                message: e.to_string(),
            })?;

        // Fetch page-level tree indexes (summaries only, no content)
        let page_trees = db::page_indexes::get_tree_indexes_for_doc(&self.db, self.kb_id, doc_id)
            .await
            .map_err(|e| GraphError::Node {
                node: "navigate_tree".into(),
                message: e.to_string(),
            })?;

        if page_trees.is_empty() {
            return Ok(json!({
                "error": "No indexed pages found for this document",
                "document_id": doc_id_str
            }));
        }

        // Build compact tree structure: summaries + topics only, no full text
        let tree_nodes: Vec<serde_json::Value> = page_trees
            .iter()
            .map(|pt| {
                let ti = &pt.tree_index;
                json!({
                    "page": pt.page_num,
                    "summary": ti.get("summary").cloned().unwrap_or(json!("")),
                    "key_entities": ti.get("key_entities").cloned().unwrap_or(json!([])),
                    "topics": ti.get("topics").cloned().unwrap_or(json!([])),
                })
            })
            .collect();

        let root_summary = root_idx
            .as_ref()
            .and_then(|r| r.root_index.get("summary"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let user_prompt = format!(
            "Document summary: {root_summary}\n\nDocument tree structure:\n{}\n\nQuestion: {query}",
            serde_json::to_string_pretty(&tree_nodes).unwrap_or_default()
        );

        let result = self
            .llm
            .complete_json(NAVIGATE_TREE_SYSTEM, &user_prompt)
            .await
            .map_err(|e| GraphError::Node {
                node: "navigate_tree".into(),
                message: format!("LLM navigation failed: {e}"),
            })?;

        // Extract the page list and return with summaries for context
        let page_list: Vec<i32> = result
            .get("page_list")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_i64().map(|n| n as i32)).collect())
            .unwrap_or_default();

        let thinking = result
            .get("thinking")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Return matched pages with their summaries
        let matched_pages: Vec<serde_json::Value> = page_list
            .iter()
            .filter_map(|&pn| {
                page_trees.iter().find(|pt| pt.page_num == pn).map(|pt| {
                    let ti = &pt.tree_index;
                    json!({
                        "page_num": pt.page_num,
                        "summary": ti.get("summary").cloned().unwrap_or(json!("")),
                    })
                })
            })
            .collect();

        Ok(json!({
            "document_id": doc_id_str,
            "thinking": thinking,
            "matched_pages": matched_pages,
            "page_count": matched_pages.len(),
        }))
    }
}

struct SearchPagesTool {
    db: PgPool,
    kb_id: Uuid,
}

#[async_trait]
impl Tool for SearchPagesTool {
    fn name(&self) -> &str {
        "search_pages"
    }
    fn description(&self) -> &str {
        "Full-text search across actual page content. Use for specific terms, acronyms, names, or phrases that may not appear in the index metadata. Returns ranked snippets with highlighted matches."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query — supports natural language, exact phrases in quotes, and boolean operators (OR, -exclude)"
                }
            },
            "required": ["query"]
        })
    }
    async fn call(&self, args: serde_json::Value) -> McResult<serde_json::Value> {
        let query = args["query"].as_str().unwrap_or("");
        if query.trim().is_empty() {
            return Ok(json!({ "error": "query is required" }));
        }

        let hits = db::page_indexes::search_pages_fts(&self.db, self.kb_id, query, 10)
            .await
            .map_err(|e| GraphError::Node {
                node: "search_pages".into(),
                message: e.to_string(),
            })?;

        let results: Vec<serde_json::Value> = hits
            .into_iter()
            .map(|h| {
                json!({
                    "document_id": h.document_id.to_string(),
                    "filename": h.filename,
                    "page_num": h.page_num,
                    "rank": h.rank,
                    "snippet": h.snippet,
                })
            })
            .collect();

        Ok(json!({ "results": results, "count": results.len() }))
    }
}

struct ReadPageTool {
    db: PgPool,
    kb_id: Uuid,
    token_budget: Arc<TokenBudget>,
}

// ---------------------------------------------------------------------------
// Token budget tracker (Win 2)
// ---------------------------------------------------------------------------

struct TokenBudget {
    used: AtomicUsize,
    max_tokens: usize,
}

impl TokenBudget {
    fn new(max_tokens: usize) -> Self {
        Self {
            used: AtomicUsize::new(0),
            max_tokens,
        }
    }

    /// Estimate tokens (~4 chars per token) and add to budget. Returns (used, remaining).
    fn consume(&self, text: &str) -> (usize, usize) {
        let tokens = text.len() / 4;
        let used = self.used.fetch_add(tokens, Ordering::Relaxed) + tokens;
        let remaining = self.max_tokens.saturating_sub(used);
        (used, remaining)
    }

    fn is_exhausted(&self) -> bool {
        self.used.load(Ordering::Relaxed) >= self.max_tokens
    }

    fn reset(&self) {
        self.used.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl Tool for ReadPageTool {
    fn name(&self) -> &str {
        "read_page"
    }
    fn description(&self) -> &str {
        "Read the full text content of a specific page from a document. Use after search_index to retrieve pages identified as relevant."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "document_id": {
                    "type": "string",
                    "description": "The document UUID"
                },
                "page_num": {
                    "type": "integer",
                    "description": "The page number to read (1-based)"
                }
            },
            "required": ["document_id", "page_num"]
        })
    }
    async fn call(&self, args: serde_json::Value) -> McResult<serde_json::Value> {
        let doc_id_str = args["document_id"]
            .as_str()
            .ok_or_else(|| GraphError::Node {
                node: "read_page".into(),
                message: "document_id required".into(),
            })?;
        let doc_id = Uuid::parse_str(doc_id_str).map_err(|e| GraphError::Node {
            node: "read_page".into(),
            message: format!("invalid document_id: {e}"),
        })?;
        let page_num = args["page_num"].as_i64().unwrap_or(1) as i32;

        // Scoped read: verify document belongs to this KB
        let page = db::page_indexes::get_page_scoped(&self.db, self.kb_id, doc_id, page_num)
            .await
            .map_err(|e| GraphError::Node {
                node: "read_page".into(),
                message: e.to_string(),
            })?;

        match page {
            Some(p) => {
                if self.token_budget.is_exhausted() {
                    return Ok(json!({
                        "warning": "Token budget exhausted. Prioritize answering with content already retrieved.",
                        "document_id": doc_id.to_string(),
                        "page_num": p.page_num,
                        "summary": p.tree_index.get("summary").cloned().unwrap_or(json!("")),
                    }));
                }

                let (used, remaining) = self.token_budget.consume(&p.content);

                let content = if remaining == 0 {
                    // Budget exceeded mid-page — truncate
                    let char_limit = self.token_budget.max_tokens.saturating_sub(used.saturating_sub(p.content.len() / 4)) * 4;
                    let truncated: String = p.content.chars().take(char_limit).collect();
                    format!("{truncated}\n\n[TRUNCATED — token budget reached]")
                } else {
                    p.content.clone()
                };

                Ok(json!({
                    "document_id": doc_id.to_string(),
                    "page_num": p.page_num,
                    "content": content,
                    "tree_index": p.tree_index,
                    "token_budget": { "used": used, "remaining": remaining },
                }))
            }
            None => Ok(json!({
                "error": format!("page {page_num} not found in document {doc_id}")
            })),
        }
    }
}

// ---------------------------------------------------------------------------
// RAG Agent builder (per-KB factory)
// ---------------------------------------------------------------------------

fn build_system_prompt(kb: &Knowledgebase) -> String {
    let mut prompt = format!(
        "You are the {} assistant that answers questions using indexed documents.\n\n",
        kb.name
    );

    if !kb.system_prompt.is_empty() {
        prompt.push_str(&kb.system_prompt);
        prompt.push_str("\n\n");
    }

    prompt.push_str(
        r#"You have three complementary search tools:

1. `search_index` — searches document metadata (entity index, page themes, keywords). Good for finding which sections/topics cover a subject.
2. `search_pages` — full-text search on actual page content. Good for specific terms, acronyms, names, exact phrases, and anything the index may have missed.
3. `navigate_tree` — LLM-powered reasoning over a document's tree structure. Best for complex or broad questions where keyword search might miss semantic connections. Requires a document_id (use `list_documents` or `search_index` first to identify the document).

Retrieval process:
1. Run `search_index` AND `search_pages` in parallel with keywords from the user's question.
2. Review the results — but treat them ONLY as a map of WHERE the answer lives. Index summaries, page themes, and search snippets are pointers, NOT the answer.
3. For complex or broad questions, use `navigate_tree` with a specific document_id to reason about which pages are relevant.
4. ALWAYS call `read_page` to read the full text of every page that plausibly contains the answer. Read multiple pages when the topic could span pages.
5. Build the answer from the actual page content you read, quoting concrete specifics.
6. If nothing matches, try synonyms, broader terms, or `list_documents` to browse what's available.

Rules for thorough answers (read before you write):
- NEVER answer from summaries or snippets alone. If you have not called `read_page`, you have not seen the answer yet — call it.
- A summary or snippet that says a page "contains a table / list / error codes / examples / steps" is proof you are NOT done: read that page and extract the actual rows, codes, values, or text.
- Give the user the specifics they asked for — exact error codes, function names, values, signatures, quoted text — not a description of where to find them. Never tell the user to "look at the document" or "refer to the table"; reading and extracting it is YOUR job.
- Always cite sources inline: document filename and page number.
- If the information spans multiple pages, read them all before answering.
- If, after actually reading the relevant pages, the content genuinely isn't there, say so honestly — do not guess.
- Read the most relevant pages in full. Only stop reading when `read_page` reports the token budget is actually exhausted — do not stop early to save calls.

Multi-turn conversations:
- You may receive prior conversation history. The user's LATEST message is your primary task.
- Treat earlier messages as context — they inform what the user is really asking about.
- When a follow-up message references something vague (e.g. "check X", "what about Y"), connect it to the original question. The user is continuing their line of inquiry, not starting a new one.
- Always prioritize answering the user's current question. Use prior context to make your answer more relevant, not to repeat previous answers."#,
    );

    prompt
}

pub struct RagAgent {
    graph: Arc<CompiledGraph<AgentState>>,
    token_budget: Arc<TokenBudget>,
}

impl RagAgent {
    pub fn new_for_kb(
        api_key: &str,
        kb: &Knowledgebase,
        db: PgPool,
    ) -> Result<Self, BoxErr> {
        let client = openai::Client::new(api_key)?;
        let model = client.completion_model(&kb.model);

        let kb_id = kb.id;
        let llm = Arc::new(
            LlmClient::new_with_model(api_key, &kb.model)
                .with_logging(db.clone(), LlmContext {
                    kb_id: Some(kb_id),
                    session_id: None,
                    request_type: "navigate_tree".to_string(),
                }),
        );

        // ~60% of 128k context for retrieved content, leaving room for
        // system prompt, history, and response generation
        let token_budget = Arc::new(TokenBudget::new(80_000));

        let registry = ToolRegistry::new()
            .register(ListDocumentsTool { db: db.clone(), kb_id })
            .register(SearchIndexTool { db: db.clone(), kb_id })
            .register(NavigateTreeTool { db: db.clone(), kb_id, llm })
            .register(SearchPagesTool { db: db.clone(), kb_id })
            .register(ReadPageTool { db, kb_id, token_budget: token_budget.clone() });

        let system_prompt = build_system_prompt(kb);
        let graph = create_react_agent(model, registry, &system_prompt)?;

        Ok(Self {
            graph: Arc::new(graph),
            token_budget,
        })
    }

    /// Build an AgentState with conversation history.
    fn build_state(&self, question: &str, history: &[ChatMessage]) -> AgentState {
        if history.is_empty() {
            return AgentState::new(question);
        }
        let first = &history[0];
        let mut s = AgentState::new(&first.content);
        for msg in &history[1..] {
            match msg.role {
                ChatRole::User => {
                    s.messages.push(AgentMessage::User(msg.content.clone()));
                }
                ChatRole::Assistant => {
                    s.messages.push(AgentMessage::Assistant(msg.content.clone()));
                }
            }
        }
        s.messages.push(AgentMessage::User(question.into()));
        s.is_done = false;
        s
    }

    /// Query with conversation history injected into agent state.
    /// Prior messages become user/assistant turns so the LLM sees full context.
    pub async fn query_with_history(
        &self,
        question: &str,
        history: &[ChatMessage],
    ) -> Result<RagResponse, BoxErr> {
        self.token_budget.reset();
        let executor = Executor::new_from_arc(self.graph.clone()).max_steps(15);
        let state = self.build_state(question, history);
        let thread_id = format!("query-{}", Uuid::new_v4());
        let outcome = executor.run(state, &thread_id).await?;
        Self::extract_response(outcome)
    }

    pub async fn query(&self, question: &str) -> Result<RagResponse, BoxErr> {
        self.token_budget.reset();
        let executor = Executor::new_from_arc(self.graph.clone()).max_steps(15);
        let state = AgentState::new(question);
        let thread_id = format!("query-{}", Uuid::new_v4());

        let outcome = executor.run(state, &thread_id).await?;
        Self::extract_response(outcome)
    }

    fn extract_response(outcome: RunOutcome<AgentState>) -> Result<RagResponse, BoxErr> {
        match outcome {
            RunOutcome::Completed(state) => {
                let answer = state
                    .final_answer()
                    .unwrap_or("No answer could be generated.")
                    .to_string();

                let tools_called = state.tools_called();
                let reasoning_path: Vec<String> = state
                    .turns()
                    .iter()
                    .filter(|t| !t.tool_calls.is_empty())
                    .map(|t| {
                        t.tool_calls
                            .iter()
                            .map(|tc| format!("{}({})", tc.name, tc.args))
                            .collect::<Vec<_>>()
                            .join(" → ")
                    })
                    .collect();

                Ok(RagResponse {
                    answer,
                    reasoning_path,
                    tools_used: tools_called,
                })
            }
            RunOutcome::Interrupted { reason, .. } => Ok(RagResponse {
                answer: format!("Query interrupted: {reason}"),
                reasoning_path: vec![],
                tools_used: vec![],
            }),
            RunOutcome::Failed { node, error, .. } => Ok(RagResponse {
                answer: format!("Query failed in '{node}': {error}"),
                reasoning_path: vec![],
                tools_used: vec![],
            }),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RagResponse {
    pub answer: String,
    pub reasoning_path: Vec<String>,
    pub tools_used: Vec<String>,
}
