use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::db;
use crate::models::chat_job::ChatJob;
use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const CONCURRENT_WORKERS: usize = 4;
const JOB_TIMEOUT: Duration = Duration::from_secs(120); // 2 minutes max per job

/// Spawn the fixed-size worker pool. Call once at startup.
pub fn spawn_workers(state: AppState) {
    let sem = Arc::new(Semaphore::new(CONCURRENT_WORKERS));

    tokio::spawn(async move {
        let worker_id = format!("worker-{}", &Uuid::new_v4().to_string()[..8]);
        tracing::info!(worker_id, workers = CONCURRENT_WORKERS, "chat worker pool started");

        loop {
            // Don't poll if all slots are busy
            if sem.available_permits() == 0 {
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }

            match db::chat_jobs::find_and_claim(&state.db, &worker_id).await {
                Ok(Some(job)) => {
                    let permit = sem.clone().acquire_owned().await.unwrap();
                    let state = state.clone();
                    let wid = worker_id.clone();
                    tokio::spawn(async move {
                        let _permit = permit; // dropped when task completes or times out
                        match tokio::time::timeout(JOB_TIMEOUT, process_job(&state, &wid, &job)).await {
                            Ok(()) => {}
                            Err(_) => {
                                tracing::error!(job_id = %job.id, "chat job timed out after {}s", JOB_TIMEOUT.as_secs());
                                // Persist an assistant message so the UI stops waiting.
                                let _ = db::chat_sessions::add_message(
                                    &state.db,
                                    job.session_id,
                                    crate::models::chat_session::ChatRole::Assistant,
                                    "Sorry — this request took too long and was stopped. Please try again or narrow your question.",
                                    None,
                                )
                                .await;
                                let _ = db::chat_jobs::fail(&state.db, job.id, &wid, "job timed out").await;
                            }
                        }
                    });
                }
                Ok(None) => {
                    // No work available, sleep before polling again
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                Err(e) => {
                    tracing::error!("chat worker poll error: {e}");
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            }
        }
    });
}

async fn process_job(state: &AppState, worker_id: &str, job: &ChatJob) {
    tracing::info!(job_id = %job.id, session_id = %job.session_id, "processing chat job");

    // Load KB for agent
    let kb = match db::knowledgebases::get_by_id(&state.db, job.kb_id).await {
        Ok(Some(kb)) => kb,
        Ok(None) => {
            let _ = db::chat_jobs::fail(&state.db, job.id, worker_id, "KB not found").await;
            return;
        }
        Err(e) => {
            let _ = db::chat_jobs::fail(&state.db, job.id, worker_id, &e.to_string()).await;
            return;
        }
    };

    // Get agent
    let agent = match state.rag_cache.get_agent(&kb).await {
        Ok(a) => a,
        Err(e) => {
            let _ = db::chat_jobs::fail(&state.db, job.id, worker_id, &e.to_string()).await;
            return;
        }
    };

    // Update session title from first query if still default
    if let Ok(Some(session)) = db::chat_sessions::get_session(&state.db, job.session_id).await {
        if session.title == "New Chat" {
            let title = crate::utils::truncate_at_char(&job.content, 50);
            let _ = db::chat_sessions::update_title(&state.db, job.session_id, &title).await;
        }
    }

    // Fetch conversation history. The current user message was already persisted
    // by the send_message handler, so it's the last row here — drop it so
    // query_with_history (which re-appends job.content as the final user turn)
    // doesn't feed the agent the same question twice.
    let mut history =
        match db::chat_sessions::get_recent_messages(&state.db, job.session_id, 20).await {
            Ok(h) => h,
            Err(e) => {
                let _ = db::chat_jobs::fail(&state.db, job.id, worker_id, &e.to_string()).await;
                return;
            }
        };
    if history
        .last()
        .is_some_and(|m| matches!(m.role, crate::models::chat_session::ChatRole::User) && m.content == job.content)
    {
        history.pop();
    }

    // Run agent
    let response = match agent.query_with_history(&job.content, &history).await {
        Ok(r) => r,
        Err(e) => {
            // Save error as assistant message so user sees it
            let _ = db::chat_sessions::add_message(
                &state.db,
                job.session_id,
                crate::models::chat_session::ChatRole::Assistant,
                &format!("Error: {e}"),
                None,
            )
            .await;
            let _ = db::chat_jobs::fail(&state.db, job.id, worker_id, &e.to_string()).await;
            return;
        }
    };

    // Save assistant response
    let meta = serde_json::json!({
        "reasoning_path": response.reasoning_path,
        "tools_used": response.tools_used,
    });

    if let Err(e) = db::chat_sessions::add_message(
        &state.db,
        job.session_id,
        crate::models::chat_session::ChatRole::Assistant,
        &response.answer,
        Some(&meta),
    )
    .await
    {
        tracing::error!(job_id = %job.id, "failed to save assistant message: {e}");
        let _ = db::chat_jobs::fail(&state.db, job.id, worker_id, &e.to_string()).await;
        return;
    }

    // Mark job complete
    if let Err(e) = db::chat_jobs::complete(&state.db, job.id, worker_id).await {
        tracing::error!(job_id = %job.id, "failed to mark job complete: {e}");
    }

    tracing::info!(job_id = %job.id, "chat job completed");
}

/// Background cleanup for stale jobs. Call in a loop from main.
pub async fn cleanup_stale_jobs(state: &AppState) {
    if let Ok(n) = db::chat_jobs::fail_stale(&state.db).await {
        if n > 0 {
            tracing::warn!("failed {n} stale in_progress chat jobs");
        }
    }
    if let Ok(n) = db::chat_jobs::fail_stale_ready(&state.db).await {
        if n > 0 {
            tracing::warn!("failed {n} stale ready chat jobs");
        }
    }
}
