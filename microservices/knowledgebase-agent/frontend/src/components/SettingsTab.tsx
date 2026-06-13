import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2, Save, Trash2 } from "lucide-react";
import { kbApi } from "../api/kb";
import type { Knowledgebase } from "../types";

export default function SettingsTab({
  kb,
  onDeleted,
}: {
  kb: Knowledgebase;
  onDeleted: () => void;
}) {
  const qc = useQueryClient();
  const [name, setName] = useState(kb.name);
  const [description, setDescription] = useState(kb.description);
  const [systemPrompt, setSystemPrompt] = useState(kb.system_prompt);
  const [model, setModel] = useState(kb.model);
  const [saved, setSaved] = useState(false);

  const save = useMutation({
    mutationFn: () =>
      kbApi.updateSettings(kb.id, {
        name,
        description,
        system_prompt: systemPrompt,
        model,
      }),
    onSuccess: (updated) => {
      qc.setQueryData(["kb", kb.id], updated);
      qc.invalidateQueries({ queryKey: ["kbs"] });
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    },
  });

  const del = useMutation({
    mutationFn: () => kbApi.deleteKb(kb.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["kbs"] });
      onDeleted();
    },
  });

  const field = "w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:border-gray-900 focus:outline-none";
  const label = "mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500";

  return (
    <div className="max-w-2xl space-y-4">
      <div>
        <label className={label}>Name</label>
        <input value={name} onChange={(e) => setName(e.target.value)} className={field} />
      </div>
      <div>
        <label className={label}>Description</label>
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          rows={2}
          className={field}
        />
      </div>
      <div>
        <label className={label}>Agent system prompt</label>
        <textarea
          value={systemPrompt}
          onChange={(e) => setSystemPrompt(e.target.value)}
          rows={4}
          placeholder="Extra instructions for the assistant (optional)"
          className={field}
        />
      </div>
      <div>
        <label className={label}>Model</label>
        <input value={model} onChange={(e) => setModel(e.target.value)} className={field} />
        <p className="mt-1 text-xs text-gray-400">OpenAI model id, e.g. gpt-4o.</p>
      </div>

      <div className="flex items-center gap-3 pt-2">
        <button
          onClick={() => save.mutate()}
          disabled={save.isPending}
          className="inline-flex items-center gap-2 rounded-lg bg-gray-900 px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
        >
          {save.isPending ? <Loader2 size={16} className="animate-spin" /> : <Save size={16} />}
          Save
        </button>
        {saved && <span className="text-sm text-emerald-600">Saved</span>}
      </div>

      <div className="mt-8 rounded-xl border border-red-200 bg-red-50 p-4">
        <p className="mb-2 text-sm font-medium text-red-700">Danger zone</p>
        <button
          onClick={() => {
            if (confirm(`Delete "${kb.name}" and all its documents? This cannot be undone.`)) {
              del.mutate();
            }
          }}
          disabled={del.isPending}
          className="inline-flex items-center gap-2 rounded-lg border border-red-300 bg-white px-4 py-2 text-sm font-medium text-red-700 hover:bg-red-100 disabled:opacity-50"
        >
          {del.isPending ? <Loader2 size={16} className="animate-spin" /> : <Trash2 size={16} />}
          Delete knowledgebase
        </button>
      </div>
    </div>
  );
}
