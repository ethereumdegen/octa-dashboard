import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileText, Loader2, RefreshCw, Trash2, Upload } from "lucide-react";
import { kbApi } from "../api/kb";
import type { DocStatus, KbDocument } from "../types";
import DocumentDetail from "./DocumentDetail";

const STATUS_STYLE: Record<DocStatus, string> = {
  uploaded: "bg-gray-100 text-gray-600",
  processing: "bg-amber-100 text-amber-700",
  indexed: "bg-emerald-100 text-emerald-700",
  failed: "bg-red-100 text-red-700",
};

function StatusBadge({ doc }: { doc: KbDocument }) {
  const label =
    doc.status === "indexed" && doc.page_count
      ? `indexed · ${doc.page_count}p`
      : doc.status;
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_STYLE[doc.status]}`}>
      {doc.status === "processing" && <Loader2 size={11} className="mr-1 inline animate-spin" />}
      {label}
    </span>
  );
}

export default function DocumentsTab({ kbId }: { kbId: string }) {
  const qc = useQueryClient();
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [openDoc, setOpenDoc] = useState<{ id: string; filename: string } | null>(null);

  const { data: docs } = useQuery({
    queryKey: ["docs", kbId],
    queryFn: () => kbApi.listDocs(kbId),
    // Poll while anything is still being indexed.
    refetchInterval: (q) => {
      const d = q.state.data as KbDocument[] | undefined;
      return d?.some((x) => x.status === "uploaded" || x.status === "processing") ? 2500 : false;
    },
  });

  const upload = useMutation({
    // Upload sequentially so one failure doesn't sink the whole batch, and
    // refresh after each so successful files appear immediately.
    mutationFn: async (files: File[]) => {
      const errors: string[] = [];
      for (const f of files) {
        try {
          await kbApi.uploadDoc(kbId, f);
          qc.invalidateQueries({ queryKey: ["docs", kbId] });
        } catch (e) {
          errors.push(`${f.name}: ${(e as Error).message}`);
        }
      }
      if (errors.length) throw new Error(errors.join(" · "));
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["docs", kbId] }),
    onError: (e: Error) => setError(e.message),
  });

  const remove = useMutation({
    mutationFn: (id: string) => kbApi.deleteDoc(kbId, id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["docs", kbId] }),
  });

  const reindex = useMutation({
    mutationFn: (id: string) => kbApi.reindexDoc(kbId, id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["docs", kbId] }),
  });

  const onFiles = (list: FileList | null) => {
    if (!list || list.length === 0) return;
    setError(null);
    upload.mutate(Array.from(list));
  };

  if (openDoc) {
    return (
      <DocumentDetail
        kbId={kbId}
        docId={openDoc.id}
        filename={openDoc.filename}
        onBack={() => setOpenDoc(null)}
      />
    );
  }

  return (
    <div>
      <div
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragOver(false);
          onFiles(e.dataTransfer.files);
        }}
        onClick={() => inputRef.current?.click()}
        className={`mb-4 flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed py-10 text-center transition ${
          dragOver ? "border-gray-900 bg-gray-50" : "border-gray-300 hover:border-gray-400"
        }`}
      >
        <input
          ref={inputRef}
          type="file"
          multiple
          accept=".md,.markdown,.txt,text/markdown,text/plain"
          className="hidden"
          onChange={(e) => onFiles(e.target.files)}
        />
        {upload.isPending ? (
          <Loader2 className="mb-2 animate-spin text-gray-400" />
        ) : (
          <Upload className="mb-2 text-gray-400" />
        )}
        <p className="text-sm font-medium">Drag &amp; drop documents, or click to browse</p>
        <p className="text-xs text-gray-400">Markdown / text files</p>
      </div>

      {error && <p className="mb-3 text-sm text-red-600">{error}</p>}

      <div className="overflow-hidden rounded-xl border border-gray-200 bg-white">
        {!docs || docs.length === 0 ? (
          <div className="py-12 text-center text-sm text-gray-400">No documents yet.</div>
        ) : (
          <ul className="divide-y divide-gray-100">
            {docs.map((doc) => (
              <li key={doc.id} className="flex items-center gap-3 px-4 py-3">
                <FileText size={18} className="shrink-0 text-gray-400" />
                <button
                  onClick={() => setOpenDoc({ id: doc.id, filename: doc.filename })}
                  className="min-w-0 flex-1 text-left"
                >
                  <p className="truncate text-sm font-medium hover:underline">{doc.filename}</p>
                  {doc.status === "failed" && doc.error_msg && (
                    <p className="truncate text-xs text-red-500">{doc.error_msg}</p>
                  )}
                </button>
                <StatusBadge doc={doc} />
                <button
                  title="Re-index"
                  onClick={() => reindex.mutate(doc.id)}
                  className="rounded p-1.5 text-gray-400 hover:bg-gray-100 hover:text-gray-700"
                >
                  <RefreshCw size={15} />
                </button>
                <button
                  title="Delete"
                  onClick={() => remove.mutate(doc.id)}
                  className="rounded p-1.5 text-gray-400 hover:bg-red-50 hover:text-red-600"
                >
                  <Trash2 size={15} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
