-- The knowledgebase microservice was rebuilt from an Obsidian-style wiki into a
-- document RAG app. Refresh its catalog description (id/name/icon unchanged).
UPDATE microservices
SET description = 'Upload documents and chat with a RAG agent over them'
WHERE id = 'knowledgebase';
