import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, FileText, Loader2, Braces } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { kbApi } from "../api/kb";
import type { PageIndex } from "../types";

/* eslint-disable @typescript-eslint/no-explicit-any */

function Chips({ items }: { items: unknown }) {
  const arr = Array.isArray(items) ? items : [];
  if (arr.length === 0) return <span className="text-xs text-gray-400">—</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {arr.map((t, i) => (
        <span key={i} className="rounded bg-gray-100 px-2 py-0.5 text-xs text-gray-600">
          {String(t)}
        </span>
      ))}
    </div>
  );
}

function PageCard({ page }: { page: PageIndex }) {
  const [open, setOpen] = useState(false);
  const ti = page.tree_index as any;
  return (
    <div className="rounded-lg border border-gray-200 bg-white">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-start gap-3 px-3 py-2.5 text-left"
      >
        <span className="mt-0.5 shrink-0 rounded bg-gray-900 px-1.5 py-0.5 text-xs font-medium text-white">
          p{page.page_num}
        </span>
        <span className="flex-1 text-sm text-gray-700">
          {ti?.summary ? String(ti.summary) : <span className="text-gray-400">No summary</span>}
        </span>
        <Braces size={14} className="mt-0.5 shrink-0 text-gray-300" />
      </button>
      {open && (
        <div className="space-y-3 border-t border-gray-100 px-3 py-3">
          {ti?.key_entities && (
            <div>
              <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400">Entities</p>
              <Chips items={ti.key_entities} />
            </div>
          )}
          {Array.isArray(ti?.topics) && ti.topics.length > 0 && (
            <div>
              <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400">Topics</p>
              <ul className="space-y-1">
                {ti.topics.map((t: any, i: number) => (
                  <li key={i} className="text-sm">
                    <span className="font-medium">{String(t?.name ?? "")}</span>
                    {t?.summary && <span className="text-gray-500"> — {String(t.summary)}</span>}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div>
            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400">Page text</p>
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded bg-gray-50 p-2 text-xs text-gray-600">
              {page.content}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}

function RagIndex({ kbId, docId }: { kbId: string; docId: string }) {
  const { data, isLoading } = useQuery({
    queryKey: ["doc-pages", kbId, docId],
    queryFn: () => kbApi.getDocPages(kbId, docId),
  });

  if (isLoading) {
    return (
      <div className="flex justify-center py-12 text-gray-400">
        <Loader2 className="animate-spin" />
      </div>
    );
  }
  if (!data) return null;

  const ri = data.root_index as any;
  const entityIndex = ri?.entity_index && typeof ri.entity_index === "object" ? ri.entity_index : {};
  const pageMap = Array.isArray(ri?.page_map) ? ri.page_map : [];

  if (!ri && data.pages.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-gray-300 py-12 text-center text-sm text-gray-400">
        Not indexed yet. The RAG metadata appears once the background indexer finishes.
      </div>
    );
  }

  return (
    <div className="space-y-5">
      {/* Document-level root index */}
      <div className="rounded-xl border border-gray-200 bg-white p-4">
        <p className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-400">Document summary</p>
        <p className="mb-4 text-sm text-gray-700">
          {ri?.summary ? String(ri.summary) : <span className="text-gray-400">—</span>}
        </p>

        <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400">Key themes</p>
        <Chips items={ri?.key_themes} />

        {Object.keys(entityIndex).length > 0 && (
          <div className="mt-4">
            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400">
              Entity index (entity → pages)
            </p>
            <div className="flex flex-wrap gap-1.5">
              {Object.entries(entityIndex).map(([entity, pages]) => (
                <span key={entity} className="rounded bg-gray-100 px-2 py-0.5 text-xs text-gray-600">
                  {entity}
                  <span className="text-gray-400"> · {(Array.isArray(pages) ? pages : []).join(", ")}</span>
                </span>
              ))}
            </div>
          </div>
        )}

        {pageMap.length > 0 && (
          <div className="mt-4">
            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-400">Page map</p>
            <ul className="space-y-1.5">
              {pageMap.map((m: any, i: number) => (
                <li key={i} className="text-sm">
                  <span className="font-medium">{String(m?.theme ?? "")}</span>
                  <span className="text-gray-400"> · pages {(Array.isArray(m?.pages) ? m.pages : []).join(", ")}</span>
                  {Array.isArray(m?.relevance_keywords) && (
                    <span className="text-gray-500"> — {m.relevance_keywords.join(", ")}</span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      {/* Per-page tree indexes */}
      <div>
        <p className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-400">
          Pages ({data.pages.length})
        </p>
        <div className="space-y-2">
          {data.pages.map((p) => (
            <PageCard key={p.id} page={p} />
          ))}
        </div>
      </div>
    </div>
  );
}

function SourceView({ kbId, docId }: { kbId: string; docId: string }) {
  const { data, isLoading, error } = useQuery({
    queryKey: ["doc-content", kbId, docId],
    queryFn: () => kbApi.getDocContent(kbId, docId),
  });

  if (isLoading) {
    return (
      <div className="flex justify-center py-12 text-gray-400">
        <Loader2 className="animate-spin" />
      </div>
    );
  }
  if (error) return <p className="text-sm text-red-600">{(error as Error).message}</p>;

  return (
    <div className="rounded-xl border border-gray-200 bg-white p-5">
      <div className="prose prose-sm max-w-none prose-pre:bg-gray-100 prose-pre:text-gray-800">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{data ?? ""}</ReactMarkdown>
      </div>
    </div>
  );
}

export default function DocumentDetail({
  kbId,
  docId,
  filename,
  onBack,
}: {
  kbId: string;
  docId: string;
  filename: string;
  onBack: () => void;
}) {
  const [tab, setTab] = useState<"source" | "index">("source");

  return (
    <div>
      <button
        onClick={onBack}
        className="mb-3 inline-flex items-center gap-1.5 text-sm text-gray-500 hover:text-gray-900"
      >
        <ArrowLeft size={15} /> Back to documents
      </button>

      <div className="mb-4 flex items-center gap-2">
        <FileText size={18} className="text-gray-400" />
        <h2 className="text-lg font-semibold">{filename}</h2>
      </div>

      <div className="mb-4 flex gap-1 border-b border-gray-200">
        {(["source", "index"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`border-b-2 px-4 py-2 text-sm font-medium transition ${
              tab === t
                ? "border-gray-900 text-gray-900"
                : "border-transparent text-gray-500 hover:text-gray-800"
            }`}
          >
            {t === "source" ? "Source" : "RAG index"}
          </button>
        ))}
      </div>

      {tab === "source" ? (
        <SourceView kbId={kbId} docId={docId} />
      ) : (
        <RagIndex kbId={kbId} docId={docId} />
      )}
    </div>
  );
}
