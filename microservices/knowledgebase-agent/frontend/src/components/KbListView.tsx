import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Book, Plus, Loader2 } from "lucide-react";
import { kbApi } from "../api/kb";

function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
}

export default function KbListView({ onSelect }: { onSelect: (id: string) => void }) {
  const qc = useQueryClient();
  const { data: kbs, isLoading } = useQuery({ queryKey: ["kbs"], queryFn: kbApi.listKbs });
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);

  const create = useMutation({
    mutationFn: () => kbApi.createKb({ name: name.trim(), slug: slugify(name), description }),
    onSuccess: (kb) => {
      qc.invalidateQueries({ queryKey: ["kbs"] });
      setShowForm(false);
      setName("");
      setDescription("");
      setError(null);
      onSelect(kb.id);
    },
    onError: (e: Error) => setError(e.message),
  });

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Knowledgebases</h1>
          <p className="text-sm text-gray-500">Upload documents and chat with an agent over them.</p>
        </div>
        <button
          onClick={() => setShowForm((v) => !v)}
          className="inline-flex items-center gap-2 rounded-lg bg-gray-900 px-4 py-2 text-sm font-medium text-white hover:bg-gray-700"
        >
          <Plus size={16} /> New knowledgebase
        </button>
      </div>

      {showForm && (
        <div className="mb-6 rounded-xl border border-gray-200 bg-white p-5">
          <div className="space-y-3">
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Name (e.g. Engineering Docs)"
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:border-gray-900 focus:outline-none"
            />
            {name && <p className="text-xs text-gray-400">slug: {slugify(name) || "—"}</p>}
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Description (optional)"
              rows={2}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:border-gray-900 focus:outline-none"
            />
            {error && <p className="text-sm text-red-600">{error}</p>}
            <div className="flex gap-2">
              <button
                disabled={!name.trim() || create.isPending}
                onClick={() => create.mutate()}
                className="inline-flex items-center gap-2 rounded-lg bg-gray-900 px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
              >
                {create.isPending && <Loader2 size={16} className="animate-spin" />} Create
              </button>
              <button
                onClick={() => setShowForm(false)}
                className="rounded-lg px-4 py-2 text-sm text-gray-600 hover:bg-gray-100"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {isLoading ? (
        <div className="flex justify-center py-16 text-gray-400">
          <Loader2 className="animate-spin" />
        </div>
      ) : !kbs || kbs.length === 0 ? (
        <div className="rounded-xl border border-dashed border-gray-300 py-16 text-center text-gray-400">
          No knowledgebases yet. Create one to get started.
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {kbs.map((kb) => (
            <button
              key={kb.id}
              onClick={() => onSelect(kb.id)}
              className="group rounded-xl border border-gray-200 bg-white p-5 text-left transition hover:border-gray-900 hover:shadow-sm"
            >
              <div className="mb-2 flex items-center gap-2">
                <span
                  className="flex h-9 w-9 items-center justify-center rounded-lg text-white"
                  style={{ background: kb.accent_color || "#111827" }}
                >
                  <Book size={18} />
                </span>
                <span className="font-semibold">{kb.name}</span>
              </div>
              <p className="line-clamp-2 text-sm text-gray-500">
                {kb.description || "No description"}
              </p>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
