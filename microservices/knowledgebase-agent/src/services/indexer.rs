use std::collections::{HashMap, HashSet};
use std::time::Duration;

use regex::Regex;
use serde_json::json;
use tokio::time::sleep;

use crate::db;
use crate::models::document::DocStatus;
use crate::services::{llm::{LlmClient, LlmContext}, s3};
use crate::state::AppState;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

/// Background worker that polls for uploaded documents globally and indexes them.
pub async fn run_indexer_loop(state: AppState) {
    tracing::info!("indexer worker started");

    let mut tick: u64 = 0;

    loop {
        // Recover stuck documents every ~60s (every 12 ticks) and on first tick
        if tick % 12 == 0 {
            match recover_stuck_documents(&state).await {
                Ok(count) if count > 0 => tracing::info!(count, "recovered stuck documents"),
                Err(e) => tracing::error!("failed to recover stuck documents: {e}"),
                _ => {}
            }
        }

        match process_pending(&state).await {
            Ok(count) => {
                if count > 0 {
                    tracing::info!(count, "indexed documents");
                }
            }
            Err(e) => {
                tracing::error!("indexer error: {e}");
            }
        }
        tick += 1;
        sleep(Duration::from_secs(15)).await;
    }
}

async fn recover_stuck_documents(state: &AppState) -> Result<usize, BoxErr> {
    let count = db::documents::reset_stuck_processing(&state.db).await?;
    Ok(count as usize)
}

async fn process_pending(state: &AppState) -> Result<usize, BoxErr> {
    let pending = db::documents::find_pending_global(&state.db).await?;
    let count = pending.len();

    for doc in pending {
        tracing::info!(doc_id = %doc.id, kb_id = %doc.kb_id, filename = %doc.filename, "processing document");

        db::documents::update_status(&state.db, doc.id, DocStatus::Processing, None).await?;

        let llm = LlmClient::new_with_model(&state.config.openai_api_key, &state.config.openai_model)
            .with_logging(state.db.clone(), LlmContext {
                kb_id: Some(doc.kb_id),
                session_id: None,
                request_type: "index_page".to_string(),
            });

        match index_document(state, &llm, &doc).await {
            Ok(()) => {
                // Only mark indexed if still in processing (a reindex may have reset it)
                db::documents::update_status_if_processing(&state.db, doc.id, DocStatus::Indexed, None).await?;
                tracing::info!(doc_id = %doc.id, "document indexed successfully");
            }
            Err(e) => {
                let msg = e.to_string();
                tracing::error!(doc_id = %doc.id, error = %msg, "indexing failed");
                db::documents::update_status_if_processing(&state.db, doc.id, DocStatus::Failed, Some(&msg))
                    .await?;
            }
        }
    }

    Ok(count)
}

async fn index_document(
    state: &AppState,
    llm: &LlmClient,
    doc: &crate::models::document::Document,
) -> Result<(), BoxErr> {
    let bucket = state.require_bucket().map_err(|e| -> BoxErr { e.to_string().into() })?;
    let bytes = s3::download_bytes(bucket, &doc.s3_key).await?;
    let text = String::from_utf8_lossy(&bytes).to_string();

    let pages = split_into_pages(&text);
    let page_count = pages.len() as i32;

    db::documents::update_page_count(&state.db, doc.id, page_count).await?;

    let mut page_summaries = Vec::new();

    for (i, page_content) in pages.iter().enumerate() {
        let page_num = (i + 1) as i32;

        tracing::info!(doc_id = %doc.id, page = page_num, total = page_count, "indexing page");

        let tree_index = build_page_tree_index(llm, page_content, page_num, &doc.filename).await?;

        db::page_indexes::insert_page(&state.db, doc.id, page_num, page_content, &tree_index)
            .await?;

        page_summaries.push(json!({
            "page": page_num,
            "summary": tree_index.get("summary").cloned().unwrap_or(json!("")),
            "topics": tree_index.get("topics").cloned().unwrap_or(json!([]))
        }));
    }

    let root_index = build_document_root_index(llm, &doc.filename, &page_summaries, &pages).await?;

    db::page_indexes::insert_document_index(&state.db, doc.id, &root_index).await?;

    // Generate wiki concept pages from the root index
    if state.bucket.is_some() {
        generate_wiki_pages(state, llm, doc, &root_index, &page_summaries).await?;
    }

    Ok(())
}

const PAGE_INDEX_SYSTEM: &str = r#"You are a document indexing system that creates structured tree indexes for document pages.

Given a page of text, produce a JSON tree index with this exact structure:
{
  "page": <page_number>,
  "summary": "<2-3 sentence summary of what this page covers>",
  "key_entities": ["<important named entities, concepts, terms>"],
  "topics": [
    {
      "name": "<topic heading>",
      "summary": "<1 sentence describing this topic>",
      "key_points": ["<important facts or claims under this topic>"],
      "subtopics": [
        {
          "name": "<subtopic>",
          "summary": "<brief description>"
        }
      ]
    }
  ],
  "relationships": ["<references to other concepts that might appear on other pages>"]
}

Rules:
- Output ONLY valid JSON, no markdown fences, no explanation
- Identify 1-5 main topics per page
- Each topic may have 0-3 subtopics
- key_entities should capture proper nouns, technical terms, important numbers
- relationships should note cross-references or dependency on other content
- If the page is mostly empty or structural (table of contents, etc.), note that in summary"#;

async fn build_page_tree_index(
    llm: &LlmClient,
    content: &str,
    page_num: i32,
    filename: &str,
) -> Result<serde_json::Value, BoxErr> {
    let user_prompt = format!(
        "Document: {filename}\nPage: {page_num}\n\n---\n{content}\n---\n\nCreate the tree index for this page.",
    );

    match llm.complete_json(PAGE_INDEX_SYSTEM, &user_prompt).await {
        Ok(index) => Ok(index),
        Err(e) => {
            tracing::warn!(page = page_num, error = %e, "LLM indexing failed, using fallback");
            Ok(build_fallback_tree_index(content, page_num))
        }
    }
}

const ROOT_INDEX_SYSTEM: &str = r#"You are a document indexing system that creates a root-level tree index summarizing an entire document.

Given page summaries, produce a JSON root index with this exact structure:
{
  "summary": "<3-5 sentence overview of the entire document>",
  "key_themes": ["<major themes across the document>"],
  "page_map": [
    {
      "pages": [1, 2],
      "theme": "<what these pages cover together>",
      "relevance_keywords": ["<keywords that would match queries about this section>"]
    }
  ],
  "entity_index": {
    "<entity_name>": [1, 3, 5]
  }
}

Rules:
- Output ONLY valid JSON, no markdown fences
- page_map groups related pages by theme (a page can appear in multiple groups)
- entity_index maps important entities to the page numbers where they appear
- relevance_keywords should include synonyms and related terms for better retrieval
- key_themes should capture 3-7 high-level themes"#;

/// Extract acronyms/abbreviations (2-6 uppercase letters) from page text.
/// Returns a map of acronym -> set of page numbers where it appears.
fn extract_acronyms_from_pages(pages: &[String]) -> HashMap<String, Vec<i64>> {
    let re = Regex::new(r"\b[A-Z][A-Z0-9]{1,5}\b").unwrap();
    // Common English words / noise to skip
    let noise: HashSet<&str> = [
        "THE", "AND", "FOR", "NOT", "BUT", "ARE", "WAS", "HAS", "HAD", "ALL",
        "CAN", "HER", "HIS", "HOW", "ITS", "MAY", "NEW", "NOW", "OLD", "OUR",
        "OUT", "OWN", "SAY", "SHE", "TOO", "USE", "WAY", "WHO", "BOY", "DID",
        "GET", "HIM", "LET", "PUT", "RUN", "TOP", "YES", "YET", "II", "III",
        "IV", "VI", "VII", "VIII", "IX", "XI", "XII", "OK",
    ].into_iter().collect();

    let mut acronyms: HashMap<String, Vec<i64>> = HashMap::new();

    for (i, page_content) in pages.iter().enumerate() {
        let page_num = (i + 1) as i64;
        let mut seen_on_page: HashSet<String> = HashSet::new();

        for m in re.find_iter(page_content) {
            let term = m.as_str().to_string();
            if !noise.contains(term.as_str()) && seen_on_page.insert(term.clone()) {
                acronyms.entry(term).or_default().push(page_num);
            }
        }
    }

    acronyms
}

/// Merge extracted acronyms into the LLM-generated entity_index.
fn merge_acronyms_into_index(index: &mut serde_json::Value, acronyms: &HashMap<String, Vec<i64>>) {
    let entity_index = index
        .as_object_mut()
        .and_then(|obj| obj.entry("entity_index").or_insert_with(|| json!({})).as_object_mut());

    if let Some(ei) = entity_index {
        for (acronym, pages) in acronyms {
            // Only add if LLM didn't already capture it (case-insensitive check)
            let already_exists = ei.keys().any(|k| k.eq_ignore_ascii_case(acronym));
            if !already_exists {
                ei.insert(acronym.clone(), json!(pages));
            }
        }
    }
}

async fn build_document_root_index(
    llm: &LlmClient,
    filename: &str,
    page_summaries: &[serde_json::Value],
    pages: &[String],
) -> Result<serde_json::Value, BoxErr> {
    let summaries_text = serde_json::to_string_pretty(page_summaries)?;
    let user_prompt = format!(
        "Document: {filename}\n\nPage summaries:\n{summaries_text}\n\nCreate the root index for this document."
    );

    let acronyms = extract_acronyms_from_pages(pages);

    match llm.complete_json(ROOT_INDEX_SYSTEM, &user_prompt).await {
        Ok(mut index) => {
            if let Some(obj) = index.as_object_mut() {
                obj.insert("filename".into(), json!(filename));
                obj.insert("page_count".into(), json!(page_summaries.len()));
            }
            merge_acronyms_into_index(&mut index, &acronyms);
            Ok(index)
        }
        Err(e) => {
            tracing::warn!(error = %e, "LLM root index failed, using fallback");
            let mut fallback = json!({
                "filename": filename,
                "page_count": page_summaries.len(),
                "summary": "Index generation failed - fallback mode",
                "pages": page_summaries,
                "entity_index": {},
            });
            merge_acronyms_into_index(&mut fallback, &acronyms);
            Ok(fallback)
        }
    }
}

const MAX_PAGE_CHARS: usize = 3000;
const MIN_PAGE_CHARS: usize = 500;

/// Check if a line looks like a section heading (markdown # or ALL CAPS line).
fn is_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        return true;
    }
    trimmed.len() > 3
        && trimmed.len() < 100
        && trimmed
            .chars()
            .all(|c| c.is_uppercase() || c.is_whitespace() || c.is_ascii_punctuation())
}

pub fn split_into_pages(text: &str) -> Vec<String> {
    // 1. Form-feed split (real PDF page markers) — keep as primary
    let ff_pages: Vec<&str> = text.split('\x0C').collect();
    if ff_pages.len() > 1 {
        return ff_pages
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    // 2. Heading-aware split: find section boundaries
    let lines: Vec<&str> = text.lines().collect();
    let mut section_starts: Vec<usize> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        if is_heading(line) {
            section_starts.push(i);
        }
    }

    if section_starts.len() > 1 {
        // Build sections from heading boundaries
        let mut sections: Vec<String> = Vec::new();
        for (idx, &start) in section_starts.iter().enumerate() {
            let end = section_starts.get(idx + 1).copied().unwrap_or(lines.len());
            let section: String = lines[start..end].join("\n");
            sections.push(section);
        }

        // Handle any content before the first heading
        if section_starts[0] > 0 {
            let preamble: String = lines[..section_starts[0]].join("\n");
            if !preamble.trim().is_empty() {
                sections.insert(0, preamble);
            }
        }

        // Merge small sections with previous, sub-split large ones
        let mut pages: Vec<String> = Vec::new();
        for section in sections {
            let trimmed = section.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.len() < MIN_PAGE_CHARS {
                // Merge with previous page if it exists and won't exceed max
                if let Some(last) = pages.last_mut() {
                    if last.len() + trimmed.len() < MAX_PAGE_CHARS {
                        last.push_str("\n\n");
                        last.push_str(&trimmed);
                        continue;
                    }
                }
            }

            if trimmed.len() > MAX_PAGE_CHARS {
                // Sub-split large sections at paragraph boundaries
                let sub_pages = split_by_paragraphs(&trimmed);
                pages.extend(sub_pages);
            } else {
                pages.push(trimmed);
            }
        }

        if !pages.is_empty() {
            return pages;
        }
    }

    // 3. Fallback: paragraph-based splitting
    split_by_paragraphs(text)
}

fn split_by_paragraphs(text: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut pages = Vec::new();
    let mut current_page = String::new();

    for para in paragraphs {
        if current_page.len() + para.len() > MAX_PAGE_CHARS && !current_page.is_empty() {
            pages.push(current_page.trim().to_string());
            current_page = String::new();
        }
        if !current_page.is_empty() {
            current_page.push_str("\n\n");
        }
        current_page.push_str(para);
    }

    if !current_page.trim().is_empty() {
        pages.push(current_page.trim().to_string());
    }

    if pages.is_empty() {
        pages.push(text.to_string());
    }

    pages
}

const WIKI_SYSTEM: &str = r#"You are a wiki page generator. Given a document's root index and page summaries, generate a concept wiki page in Markdown.

The page should:
1. Have a clear title as H1
2. Provide a comprehensive explanation of the concept
3. Include relevant details from the source document
4. Use proper Markdown formatting (headers, lists, code blocks if relevant)
5. End with a "Sources" section citing the document and page numbers

Output ONLY the Markdown content, no JSON wrapping."#;

async fn generate_wiki_pages(
    state: &AppState,
    llm: &LlmClient,
    doc: &crate::models::document::Document,
    root_index: &serde_json::Value,
    page_summaries: &[serde_json::Value],
) -> Result<(), BoxErr> {
    let bucket = state.require_bucket().map_err(|e| -> BoxErr { e.to_string().into() })?;

    // Extract key themes from root index to generate wiki pages
    let themes = root_index
        .get("key_themes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for theme_val in themes.iter().take(5) {
        let theme = match theme_val.as_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        let slug = theme
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string();

        if slug.is_empty() {
            continue;
        }

        let user_prompt = format!(
            "Document: {}\n\nRoot Index:\n{}\n\nPage Summaries:\n{}\n\nGenerate a wiki concept page about: {}",
            doc.filename,
            serde_json::to_string_pretty(root_index).unwrap_or_default(),
            serde_json::to_string_pretty(page_summaries).unwrap_or_default(),
            theme,
        );

        let markdown = match llm.complete(WIKI_SYSTEM, &user_prompt).await {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(theme = %theme, error = %e, "wiki page generation failed, skipping");
                continue;
            }
        };

        let s3_key = format!("{}/wiki/{}.md", doc.kb_id, slug);
        s3::upload_bytes(bucket, &s3_key, markdown.as_bytes(), "text/markdown").await?;

        let sources = json!([{
            "document_id": doc.id,
            "filename": doc.filename,
        }]);

        db::wiki_pages::upsert(
            &state.db,
            doc.kb_id,
            Some(doc.id),
            &slug,
            &theme,
            root_index.get("summary").and_then(|v| v.as_str()),
            &s3_key,
            "concept",
            &sources,
        )
        .await?;

        tracing::info!(doc_id = %doc.id, slug = %slug, "wiki page generated");
    }

    Ok(())
}

fn build_fallback_tree_index(content: &str, page_num: i32) -> serde_json::Value {
    let lines: Vec<&str> = content.lines().collect();

    let topics: Vec<serde_json::Value> = lines
        .iter()
        .filter(|line| is_heading(line))
        .map(|line| {
            json!({
                "name": line.trim().trim_start_matches('#').trim(),
                "summary": "",
                "key_points": [],
                "subtopics": []
            })
        })
        .collect();

    let truncated = if content.len() <= 300 {
        content.to_string()
    } else {
        format!("{}...", &content[..300])
    };

    json!({
        "page": page_num,
        "summary": truncated,
        "key_entities": [],
        "topics": topics,
        "relationships": []
    })
}
