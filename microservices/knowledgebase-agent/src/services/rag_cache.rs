use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db;
use crate::models::knowledgebase::Knowledgebase;

use super::rag_agent::RagAgent;

const EVICTION_TTL: Duration = Duration::from_secs(30 * 60); // 30 minutes

struct CacheEntry {
    agent: Arc<RagAgent>,
    last_used: Instant,
}

pub struct RagCache {
    cache: RwLock<HashMap<Uuid, CacheEntry>>,
    db: PgPool,
    openai_api_key: String,
}

impl RagCache {
    pub fn new(db: PgPool, openai_api_key: String) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            db,
            openai_api_key,
        }
    }

    pub async fn get_agent(&self, kb: &Knowledgebase) -> Result<Arc<RagAgent>, Box<dyn std::error::Error + Send + Sync>> {
        // Check cache
        {
            let mut cache = self.cache.write().await;
            if let Some(entry) = cache.get_mut(&kb.id) {
                entry.last_used = Instant::now();
                return Ok(entry.agent.clone());
            }
        }

        // Build new agent
        let agent = RagAgent::new_for_kb(&self.openai_api_key, kb, self.db.clone())?;
        let agent = Arc::new(agent);

        // Insert into cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(kb.id, CacheEntry {
                agent: agent.clone(),
                last_used: Instant::now(),
            });
        }

        Ok(agent)
    }

    pub async fn invalidate(&self, kb_id: Uuid) {
        let mut cache = self.cache.write().await;
        cache.remove(&kb_id);
    }

    pub async fn evict_stale(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();
        cache.retain(|_, entry| now.duration_since(entry.last_used) < EVICTION_TTL);
    }

    /// Retrieve endpoint: search + read_page without LLM synthesis.
    pub async fn retrieve(
        &self,
        kb_id: Uuid,
        query: &str,
        max_pages: usize,
    ) -> Result<Vec<RetrievedDocument>, Box<dyn std::error::Error + Send + Sync>> {
        // Get all document indexes for this KB
        let indexes = db::page_indexes::get_document_indexes_for_kb(&self.db, kb_id).await?;

        // Score pages by simple keyword overlap on the root index
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut page_candidates: Vec<(Uuid, i32, f64)> = Vec::new();

        for idx in &indexes {
            let root_str = idx.root_index.to_string().to_lowercase();
            let doc_score: f64 = query_words
                .iter()
                .filter(|w| root_str.contains(*w))
                .count() as f64;

            if doc_score > 0.0 {
                // Look at page_map for more specific matches
                if let Some(page_map) = idx.root_index.get("page_map").and_then(|v| v.as_array()) {
                    for entry in page_map {
                        let entry_str = entry.to_string().to_lowercase();
                        let page_score: f64 = query_words
                            .iter()
                            .filter(|w| entry_str.contains(*w))
                            .count() as f64;

                        if page_score > 0.0 {
                            if let Some(pages) = entry.get("pages").and_then(|v| v.as_array()) {
                                for p in pages {
                                    if let Some(pn) = p.as_i64() {
                                        page_candidates.push((idx.document_id, pn as i32, page_score + doc_score));
                                    }
                                }
                            }
                        }
                    }
                }

                // If no page_map hits, add all pages with doc-level score
                if !page_candidates.iter().any(|(did, _, _)| *did == idx.document_id) {
                    if let Some(pc) = idx.root_index.get("page_count").and_then(|v| v.as_i64()) {
                        for pn in 1..=pc {
                            page_candidates.push((idx.document_id, pn as i32, doc_score));
                        }
                    }
                }
            }
        }

        // Sort by score descending, take max_pages
        page_candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        page_candidates.truncate(max_pages);

        // Fetch actual page content
        let mut results: HashMap<Uuid, RetrievedDocument> = HashMap::new();

        for (doc_id, page_num, score) in page_candidates {
            if let Some(page) = db::page_indexes::get_page_scoped(&self.db, kb_id, doc_id, page_num).await? {
                let entry = results.entry(doc_id).or_insert_with(|| {
                    // Look up document filename
                    RetrievedDocument {
                        id: doc_id,
                        title: String::new(),
                        pages: Vec::new(),
                    }
                });
                entry.pages.push(RetrievedPage {
                    page_num: page.page_num,
                    content: page.content,
                    tree_index: page.tree_index,
                    relevance: score,
                });
            }
        }

        // Fill in titles
        for (doc_id, doc) in results.iter_mut() {
            if let Ok(Some(d)) = db::documents::get_by_id(&self.db, *doc_id).await {
                doc.title = d.filename;
            }
        }

        Ok(results.into_values().collect())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RetrievedDocument {
    pub id: Uuid,
    pub title: String,
    pub pages: Vec<RetrievedPage>,
}

#[derive(Debug, serde::Serialize)]
pub struct RetrievedPage {
    pub page_num: i32,
    pub content: String,
    pub tree_index: serde_json::Value,
    pub relevance: f64,
}
