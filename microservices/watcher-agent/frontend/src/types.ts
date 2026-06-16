export interface Repo {
  id: string;
  owner: string;
  repo: string;
  default_branch: string;
  enabled: boolean;
  last_checked_at: string | null;
  last_error: string | null;
  created_at: string;
}

export interface ChangedFile {
  filename: string;
  status: string;
  additions: number;
  deletions: number;
}

export interface Commit {
  id: string;
  repo_id: string;
  sha: string;
  author: string | null;
  author_email: string | null;
  message: string | null;
  url: string | null;
  committed_at: string | null;
  additions: number | null;
  deletions: number | null;
  files_changed: ChangedFile[] | null;
  summary: string | null;
  summary_status: "pending" | "done" | "error";
  raw_data?: unknown;
  created_at: string;
  repo_owner?: string;
  repo_name?: string;
}
