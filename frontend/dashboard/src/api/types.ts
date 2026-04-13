export interface User {
  id: string;
  email: string;
  role: "admin" | "member";
}

export interface TeamMember {
  id: string;
  email: string;
  role: string;
  invited_by: string | null;
  created_at: string;
  last_active: string | null;
}

export interface Agent {
  id: string;
  name: string;
  url: string;
  manifest: AgentManifest;
  status: string;
  last_health_check: string | null;
  registered_at: string;
}

export interface AgentManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  icon?: string;
  ui?: { entry_path: string; width?: number; height?: number; bundle_js?: string; bundle_css?: string };
  api?: { base_path: string };
  health_endpoint: string;
  capabilities: string[];
  required_secrets?: string[];
}

export interface ApiKey {
  id: string;
  name: string;
  key_prefix: string;
  key_suffix: string;
  key?: string; // Only present on creation response
  created_at: string;
  last_used_at: string | null;
}

export interface AnalyticsSummary {
  total_users: number;
  total_agents: number;
  total_documents: number;
  logins_today: number;
}

export interface Microservice {
  id: string;
  name: string;
  description: string;
  icon: string;
  slug: string;
  nav_path: string;
  enabled: boolean;
  source_url?: string;
  installed_at: string;
}

export interface ServiceInfo {
  id: string;
  name: string;
  description: string;
  icon: string;
  slug: string;
  nav_path: string;
  enabled: boolean;
  source_url?: string;
  installed_at: string;
  agent_status?: string;
  agent_url?: string;
  last_health_check?: string;
  manifest?: AgentManifest;
  required_secrets?: string[];
  missing_secrets?: string[];
}

export interface ChartData {
  metric: string;
  days: number;
  data: { date: string; count: number }[];
}
