import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { KbDocument, VaultExport, ImportResult } from "../types";

export function useDocuments() {
  return useQuery<KbDocument[]>({
    queryKey: ["kb", "documents"],
    queryFn: () => api.get("/api/documents"),
  });
}

export function useDocument(id: string | null) {
  return useQuery<KbDocument>({
    queryKey: ["kb", "document", id],
    queryFn: () => api.get(`/api/documents/${id}`),
    enabled: !!id,
  });
}

export function useDocumentMutations() {
  const qc = useQueryClient();

  const create = useMutation({
    mutationFn: (body: {
      title: string;
      slug: string;
      content?: string;
      parent_id?: string;
      is_folder?: boolean;
    }) => api.post<KbDocument>("/api/documents", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["kb"] }),
  });

  const update = useMutation({
    mutationFn: ({
      id,
      ...body
    }: {
      id: string;
      title?: string;
      slug?: string;
      content?: string;
      parent_id?: string;
    }) => api.put<KbDocument>(`/api/documents/${id}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["kb"] }),
  });

  const remove = useMutation({
    mutationFn: (id: string) => api.delete(`/api/documents/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["kb"] }),
  });

  return { create, update, remove };
}

export function useKbExport() {
  return useMutation({
    mutationFn: () => api.get<VaultExport>("/api/export"),
  });
}

export function useKbImport() {
  const qc = useQueryClient();

  return useMutation({
    mutationFn: (data: VaultExport) => api.post<ImportResult>("/api/import", data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["kb"] }),
  });
}
