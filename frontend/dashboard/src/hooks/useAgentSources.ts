import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";

export interface AgentSource {
  id: string;
  url: string;
  label: string;
  created_at: string;
}

export function useAgentSources() {
  return useQuery<AgentSource[]>({
    queryKey: ["agent-sources"],
    queryFn: () => api.get("/agent-sources"),
  });
}

export function useAgentSourceMutations() {
  const qc = useQueryClient();

  const add = useMutation({
    mutationFn: (body: { url: string; label?: string }) =>
      api.post<AgentSource>("/agent-sources", body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-sources"] });
      qc.invalidateQueries({ queryKey: ["agents"] });
      qc.invalidateQueries({ queryKey: ["microservices"] });
      qc.invalidateQueries({ queryKey: ["services"] });
    },
  });

  const remove = useMutation({
    mutationFn: (id: string) => api.delete(`/agent-sources/${id}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-sources"] });
      qc.invalidateQueries({ queryKey: ["agents"] });
      qc.invalidateQueries({ queryKey: ["microservices"] });
      qc.invalidateQueries({ queryKey: ["services"] });
    },
  });

  return { add, remove };
}
