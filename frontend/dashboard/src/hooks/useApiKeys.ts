import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { ApiKey } from "../api/types";

export function useApiKeys() {
  return useQuery<ApiKey[]>({
    queryKey: ["api-keys"],
    queryFn: () => api.get("/api-keys"),
  });
}

export function useApiKeyMutations() {
  const qc = useQueryClient();

  const create = useMutation({
    mutationFn: (body: { name: string }) =>
      api.post<ApiKey & { key: string }>("/api-keys", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["api-keys"] }),
  });

  const revoke = useMutation({
    mutationFn: (id: string) => api.delete(`/api-keys/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["api-keys"] }),
  });

  return { create, revoke };
}
