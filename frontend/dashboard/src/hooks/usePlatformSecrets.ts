import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";

export interface PlatformSecret {
  key: string;
  description: string;
  is_set: boolean;
  updated_at: string | null;
}

export function usePlatformSecrets() {
  return useQuery<PlatformSecret[]>({
    queryKey: ["platform-secrets"],
    queryFn: () => api.get("/platform-secrets"),
  });
}

export function usePlatformSecretMutations() {
  const qc = useQueryClient();

  const create = useMutation({
    mutationFn: (body: { key: string; value: string; description?: string }) =>
      api.post("/platform-secrets", body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["platform-secrets"] });
      qc.invalidateQueries({ queryKey: ["services"] });
    },
  });

  const set = useMutation({
    mutationFn: ({ key, value, description }: { key: string; value: string; description?: string }) =>
      api.put(`/platform-secrets/${key}`, { value, description }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["platform-secrets"] });
      qc.invalidateQueries({ queryKey: ["services"] });
    },
  });

  const remove = useMutation({
    mutationFn: (key: string) => api.delete(`/platform-secrets/${key}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["platform-secrets"] });
      qc.invalidateQueries({ queryKey: ["services"] });
    },
  });

  return { create, set, remove };
}
