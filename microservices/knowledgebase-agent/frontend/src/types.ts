export interface Knowledgebase {
  id: string;
  name: string;
  slug: string;
  description: string;
  system_prompt: string;
  model: string;
  accent_color: string;
  logo_url: string | null;
  created_by: string | null;
  created_at: string;
  updated_at: string;
}

export type DocStatus = "uploaded" | "processing" | "indexed" | "failed";

export interface KbDocument {
  id: string;
  kb_id: string;
  filename: string;
  mime_type: string;
  s3_key: string;
  size_bytes: number;
  status: DocStatus;
  folder_id: string | null;
  page_count: number | null;
  pages_indexed: number | null;
  error_msg: string | null;
  uploaded_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface ChatSession {
  id: string;
  kb_id: string;
  user_id: string | null;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface ChatMessageMeta {
  reasoning_path?: string[];
  tools_used?: string[];
}

export interface ChatMessage {
  id: string;
  session_id: string;
  role: "user" | "assistant";
  content: string;
  metadata: ChatMessageMeta | null;
  created_at: string;
}

export interface SessionDetail {
  session: ChatSession;
  messages: ChatMessage[];
}

/** A page-level PageIndex row (chunk text + the LLM-generated tree index). */
export interface PageIndex {
  id: string;
  document_id: string;
  page_num: number;
  content: string;
  // LLM-generated structure: { summary, key_entities[], topics[], relationships[] }
  tree_index: Record<string, unknown>;
  created_at: string;
}

/** Response of GET /documents/:id/pages — the RAG metadata for a document. */
export interface DocPages {
  document: KbDocument;
  pages: PageIndex[];
  // Document-level root index: { summary, key_themes[], page_map[], entity_index{} }
  root_index: Record<string, unknown> | null;
}
