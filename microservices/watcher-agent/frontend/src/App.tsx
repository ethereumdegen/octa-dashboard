import { useState } from "react";
import {
  ArrowLeft,
  Eye,
  GitCommit,
  Plus,
  RefreshCw,
  Trash2,
  ExternalLink,
} from "lucide-react";
import {
  useAddRepo,
  useCommit,
  useCommits,
  useDeleteRepo,
  useRepos,
  useTrigger,
} from "./hooks/useWatcher";
import type { Commit } from "./types";

function shortSha(sha: string) {
  return sha.slice(0, 7);
}

function timeAgo(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso).getTime();
  const secs = Math.floor((Date.now() - d) / 1000);
  if (secs < 60) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

function firstLine(msg: string | null): string {
  return (msg ?? "").split("\n")[0] || "(no message)";
}

function SummaryBadge({ status }: { status: Commit["summary_status"] }) {
  const map: Record<string, string> = {
    pending: "bg-amber-100 text-amber-700",
    done: "bg-emerald-100 text-emerald-700",
    error: "bg-rose-100 text-rose-700",
  };
  const label = { pending: "summarizing", done: "summarized", error: "summary failed" }[status];
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${map[status]}`}>
      {label}
    </span>
  );
}

// ── Repo manager ────────────────────────────────────────────────────────

function RepoManager() {
  const { data: repos } = useRepos();
  const addRepo = useAddRepo();
  const deleteRepo = useDeleteRepo();
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const match = value.trim().match(/^([\w.-]+)\/([\w.-]+)$/);
    if (!match) {
      setError("Use owner/repo format, e.g. teller-protocol/teller-app");
      return;
    }
    addRepo.mutate(
      { owner: match[1], repo: match[2] },
      {
        onSuccess: () => setValue(""),
        onError: (err) => setError(String(err)),
      },
    );
  }

  return (
    <div className="rounded-2xl bg-card p-5 shadow-sm">
      <h2 className="mb-3 text-sm font-semibold text-gray-900">Watched repositories</h2>
      <form onSubmit={submit} className="mb-4 flex gap-2">
        <input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="owner/repo"
          className="flex-1 rounded-lg border border-gray-200 px-3 py-2 text-sm outline-none focus:border-indigo-400"
        />
        <button
          type="submit"
          disabled={addRepo.isPending}
          className="flex items-center gap-1 rounded-lg bg-indigo-600 px-3 py-2 text-sm font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
        >
          <Plus size={16} /> Add
        </button>
      </form>
      {error && <p className="mb-3 text-xs text-rose-600">{error}</p>}
      <ul className="space-y-2">
        {repos?.length === 0 && (
          <li className="text-sm text-gray-400">No repositories yet — add one above.</li>
        )}
        {repos?.map((r) => (
          <li
            key={r.id}
            className="flex items-center justify-between rounded-lg border border-gray-100 px-3 py-2"
          >
            <div className="min-w-0">
              <div className="truncate text-sm font-medium text-gray-800">
                {r.owner}/{r.repo}
                <span className="ml-2 text-xs font-normal text-gray-400">{r.default_branch}</span>
              </div>
              <div className="text-xs text-gray-400">
                {r.last_error ? (
                  <span className="text-rose-500">{r.last_error}</span>
                ) : (
                  <>checked {timeAgo(r.last_checked_at)}</>
                )}
              </div>
            </div>
            <button
              onClick={() => deleteRepo.mutate(r.id)}
              className="rounded-md p-1.5 text-gray-400 hover:bg-rose-50 hover:text-rose-600"
              title="Remove"
            >
              <Trash2 size={16} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ── Commit index ──────────────────────────────────────────────────────────

function CommitIndex({ onOpen }: { onOpen: (id: string) => void }) {
  const { data: commits, isLoading } = useCommits();
  const trigger = useTrigger();

  return (
    <div className="grid grid-cols-1 gap-5 lg:grid-cols-[1fr_320px]">
      <div className="rounded-2xl bg-card p-5 shadow-sm">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-sm font-semibold text-gray-900">Recent commits</h2>
          <button
            onClick={() => trigger.mutate()}
            disabled={trigger.isPending}
            className="flex items-center gap-1 rounded-lg border border-gray-200 px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-50 disabled:opacity-50"
          >
            <RefreshCw size={14} className={trigger.isPending ? "animate-spin" : ""} />
            Check now
          </button>
        </div>

        {isLoading && <p className="text-sm text-gray-400">Loading…</p>}
        {commits?.length === 0 && (
          <p className="text-sm text-gray-400">
            No commits yet. The watcher polls every 5 minutes, or hit “Check now”.
          </p>
        )}

        <ul className="divide-y divide-gray-100">
          {commits?.map((c) => (
            <li key={c.id}>
              <button
                onClick={() => onOpen(c.id)}
                className="flex w-full items-start gap-3 py-3 text-left hover:bg-gray-50"
              >
                <GitCommit size={18} className="mt-0.5 shrink-0 text-gray-400" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium text-gray-900">
                      {firstLine(c.message)}
                    </span>
                  </div>
                  <div className="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-gray-500">
                    <span className="font-mono text-gray-400">{shortSha(c.sha)}</span>
                    {c.repo_owner && (
                      <span>
                        {c.repo_owner}/{c.repo_name}
                      </span>
                    )}
                    <span>· {c.author ?? "unknown"}</span>
                    <span>· {timeAgo(c.committed_at)}</span>
                    <SummaryBadge status={c.summary_status} />
                  </div>
                  {c.summary && (
                    <p className="mt-1 line-clamp-2 text-xs text-gray-500">{c.summary}</p>
                  )}
                </div>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <RepoManager />
    </div>
  );
}

// ── Commit show page ────────────────────────────────────────────────────

function CommitShow({ id, onBack }: { id: string; onBack: () => void }) {
  const { data: c, isLoading } = useCommit(id);

  if (isLoading || !c) {
    return <p className="text-sm text-gray-400">Loading…</p>;
  }

  return (
    <div className="mx-auto max-w-3xl">
      <button
        onClick={onBack}
        className="mb-4 flex items-center gap-1 text-sm text-gray-500 hover:text-gray-800"
      >
        <ArrowLeft size={16} /> Back to commits
      </button>

      <div className="rounded-2xl bg-card p-6 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <span className="font-mono text-sm text-gray-400">{shortSha(c.sha)}</span>
          <SummaryBadge status={c.summary_status} />
          {c.url && (
            <a
              href={c.url}
              target="_blank"
              rel="noreferrer"
              className="ml-auto flex items-center gap-1 text-xs text-indigo-600 hover:underline"
            >
              View on GitHub <ExternalLink size={12} />
            </a>
          )}
        </div>

        <h1 className="mb-1 whitespace-pre-wrap text-lg font-semibold text-gray-900">
          {firstLine(c.message)}
        </h1>
        <div className="mb-4 text-xs text-gray-500">
          {c.repo_owner}/{c.repo_name} · {c.author ?? "unknown"}
          {c.author_email ? ` <${c.author_email}>` : ""} · {timeAgo(c.committed_at)}
        </div>

        {c.summary && (
          <div className="mb-5 rounded-xl bg-indigo-50 p-4">
            <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-indigo-500">
              AI summary
            </div>
            <p className="text-sm text-gray-700">{c.summary}</p>
          </div>
        )}

        {c.message && c.message.includes("\n") && (
          <pre className="mb-5 whitespace-pre-wrap rounded-xl bg-gray-50 p-4 text-sm text-gray-700">
            {c.message.trim()}
          </pre>
        )}

        <div className="mb-3 flex gap-4 text-sm">
          <span className="text-emerald-600">+{c.additions ?? 0}</span>
          <span className="text-rose-600">−{c.deletions ?? 0}</span>
          <span className="text-gray-400">
            {c.files_changed?.length ?? 0} file(s) changed
          </span>
        </div>

        <ul className="divide-y divide-gray-100 rounded-xl border border-gray-100">
          {c.files_changed?.map((f) => (
            <li key={f.filename} className="flex items-center justify-between px-3 py-2 text-sm">
              <span className="truncate font-mono text-xs text-gray-700">{f.filename}</span>
              <span className="ml-3 shrink-0 text-xs">
                <span className="text-emerald-600">+{f.additions}</span>{" "}
                <span className="text-rose-600">−{f.deletions}</span>
              </span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

// ── Root ──────────────────────────────────────────────────────────────────

export default function App() {
  const [selected, setSelected] = useState<string | null>(null);

  return (
    <div className="mx-auto max-w-5xl">
      <header className="mb-6 flex items-center gap-2">
        <Eye size={22} className="text-indigo-600" />
        <h1 className="text-xl font-bold text-gray-900">The Watcher</h1>
        <span className="text-sm text-gray-400">· recent GitHub commits</span>
      </header>

      {selected ? (
        <CommitShow id={selected} onBack={() => setSelected(null)} />
      ) : (
        <CommitIndex onOpen={setSelected} />
      )}
    </div>
  );
}
