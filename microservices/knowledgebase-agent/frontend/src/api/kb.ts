import { api } from "./client";
import type {
  ChatMessage,
  ChatSession,
  DocPages,
  KbDocument,
  Knowledgebase,
  SessionDetail,
} from "../types";

export interface CreateKbInput {
  name: string;
  slug: string;
  description?: string;
}

export interface UpdateSettingsInput {
  name?: string;
  description?: string;
  system_prompt?: string;
  model?: string;
  accent_color?: string;
}

export const kbApi = {
  listKbs: () => api.get<Knowledgebase[]>("/api/kbs"),
  createKb: (body: CreateKbInput) => api.post<Knowledgebase>("/api/kbs", body),
  getKb: (id: string) => api.get<Knowledgebase>(`/api/kb/${id}`),
  deleteKb: (id: string) => api.delete<void>(`/api/kb/${id}`),
  updateSettings: (id: string, body: UpdateSettingsInput) =>
    api.put<Knowledgebase>(`/api/kb/${id}/settings`, body),

  listDocs: (kbId: string) => api.get<KbDocument[]>(`/api/kb/${kbId}/documents`),
  uploadDoc: (kbId: string, file: File) => {
    const fd = new FormData();
    fd.append("file", file);
    return api.upload<KbDocument>(`/api/kb/${kbId}/documents`, fd);
  },
  deleteDoc: (kbId: string, id: string) => api.delete<void>(`/api/kb/${kbId}/documents/${id}`),
  reindexDoc: (kbId: string, id: string) =>
    api.post<KbDocument>(`/api/kb/${kbId}/documents/${id}/reindex`),
  getDocContent: (kbId: string, id: string) =>
    api.getText(`/api/kb/${kbId}/documents/${id}/content`),
  getDocPages: (kbId: string, id: string) =>
    api.get<DocPages>(`/api/kb/${kbId}/documents/${id}/pages`),

  listSessions: (kbId: string) => api.get<ChatSession[]>(`/api/kb/${kbId}/sessions`),
  createSession: (kbId: string, title?: string) =>
    api.post<ChatSession>(`/api/kb/${kbId}/sessions`, { title }),
  getSession: (kbId: string, sid: string) =>
    api.get<SessionDetail>(`/api/kb/${kbId}/sessions/${sid}`),
  deleteSession: (kbId: string, sid: string) =>
    api.delete<void>(`/api/kb/${kbId}/sessions/${sid}`),
  sendMessage: (kbId: string, sid: string, content: string) =>
    api.post<ChatMessage>(`/api/kb/${kbId}/sessions/${sid}/messages`, { content }),
};
