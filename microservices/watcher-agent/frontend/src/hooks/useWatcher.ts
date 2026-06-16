import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { Commit, Repo } from "../types";

// ── Commits ───────────────────────────────────────────────────────────────

export function useCommits() {
  return useQuery<Commit[]>({
    queryKey: ["watcher", "commits"],
    queryFn: () => api.get("/api/commits"),
    refetchInterval: 15000,
  });
}

export function useCommit(id: string | null) {
  return useQuery<Commit>({
    queryKey: ["watcher", "commit", id],
    queryFn: () => api.get(`/api/commits/${id}`),
    enabled: !!id,
    refetchInterval: 15000,
  });
}

export function useTrigger() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post("/api/trigger"),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["watcher"] }),
  });
}

// ── Repos ─────────────────────────────────────────────────────────────────

export function useRepos() {
  return useQuery<Repo[]>({
    queryKey: ["watcher", "repos"],
    queryFn: () => api.get("/api/repos"),
    refetchInterval: 30000,
  });
}

export function useAddRepo() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { owner: string; repo: string }) =>
      api.post<Repo>("/api/repos", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["watcher"] }),
  });
}

export function useDeleteRepo() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/repos/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["watcher"] }),
  });
}
