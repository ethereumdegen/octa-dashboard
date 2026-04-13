export interface KbDocument {
  id: string;
  title: string;
  slug: string;
  content: string;
  parent_id: string | null;
  is_folder: boolean;
  sort_order: number;
  created_by: string | null;
  updated_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface VaultExport {
  version: number;
  exported_at: string;
  documents: VaultDocument[];
}

export interface VaultDocument {
  path: string;
  content: string;
  is_folder: boolean;
}

export interface ImportResult {
  imported: number;
  errors: string[];
}
