// Base path for API requests, set by mount() when embedded in the dashboard.
let basePath = "";

export function setBasePath(path: string) {
  basePath = path;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${basePath}${path}`, {
    credentials: "include",
    headers: { "Content-Type": "application/json", ...init?.headers },
    ...init,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Request failed: ${res.status}`);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

export const api = {
  get: <T>(path: string) => request<T>(path),

  /** Fetch a raw text/markdown body (e.g. a document's original content). */
  getText: async (path: string): Promise<string> => {
    const res = await fetch(`${basePath}${path}`, { credentials: "include" });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error || `Request failed: ${res.status}`);
    }
    return res.text();
  },

  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: "POST", body: body ? JSON.stringify(body) : undefined }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: "PUT", body: body ? JSON.stringify(body) : undefined }),
  delete: <T>(path: string) => request<T>(path, { method: "DELETE" }),

  /** Multipart upload — no Content-Type header so the browser sets the boundary. */
  upload: async <T>(path: string, formData: FormData): Promise<T> => {
    const res = await fetch(`${basePath}${path}`, {
      method: "POST",
      credentials: "include",
      body: formData,
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error || `Upload failed: ${res.status}`);
    }
    return res.json();
  },
};
