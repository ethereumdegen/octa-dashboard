-- The stub knowledgebase agent stored markdown docs in `kb_documents`
-- (migrations 004 + 007). The agent has been replaced by the document-RAG
-- knowledgebase (migration 017), which uses its own tables. Drop the now-unused
-- legacy table so it doesn't linger as dead schema.
DROP TABLE IF EXISTS kb_documents CASCADE;
