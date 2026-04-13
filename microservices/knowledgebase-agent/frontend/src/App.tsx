import { useState, useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  FileText,
  Folder,
  FolderPlus,
  Plus,
  Trash2,
  Eye,
  Edit2,
  Download,
  Upload,
  ChevronRight,
  ChevronDown,
  ArrowLeft,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import {
  useDocuments,
  useDocument,
  useDocumentMutations,
  useKbExport,
  useKbImport,
} from "./hooks/useDocuments";
import type { KbDocument, VaultExport } from "./types";

export default function App() {
  const { data: documents, isLoading } = useDocuments();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { data: selectedDoc } = useDocument(selectedId);
  const { create, update, remove } = useDocumentMutations();
  const exportMutation = useKbExport();
  const importMutation = useKbImport();
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState("");
  const [editContent, setEditContent] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (selectedDoc) {
      setEditTitle(selectedDoc.title);
      setEditContent(selectedDoc.content);
    }
  }, [selectedDoc]);

  const handleCreate = (parentId?: string) => {
    const title = "New Document";
    const slug = `new-doc-${Date.now()}`;
    create.mutate(
      { title, slug, content: "", parent_id: parentId },
      {
        onSuccess: (doc: KbDocument) => {
          setSelectedId(doc.id);
          setEditing(true);
        },
      }
    );
  };

  const handleCreateFolder = (parentId?: string) => {
    const title = "New Folder";
    const slug = `new-folder-${Date.now()}`;
    create.mutate(
      { title, slug, content: "", parent_id: parentId, is_folder: true },
      {
        onSuccess: (doc: KbDocument) => {
          setSelectedId(doc.id);
        },
      }
    );
  };

  const handleSave = () => {
    if (!selectedId) return;
    update.mutate({ id: selectedId, title: editTitle, content: editContent });
    setEditing(false);
  };

  const handleDelete = (id: string) => {
    const doc = documents?.find((d) => d.id === id);
    const children = documents?.filter((d) => d.parent_id === id) ?? [];
    const msg = doc?.is_folder && children.length > 0
      ? `Delete folder "${doc.title}" and orphan its ${children.length} children?`
      : `Delete "${doc?.title ?? "this document"}"?`;
    if (!confirm(msg)) return;
    remove.mutate(id);
    if (selectedId === id) setSelectedId(null);
  };

  const handleExport = () => {
    exportMutation.mutate(undefined, {
      onSuccess: (data) => {
        const blob = new Blob([JSON.stringify(data, null, 2)], {
          type: "application/json",
        });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `knowledgebase-export-${new Date().toISOString().split("T")[0]}.json`;
        a.click();
        URL.revokeObjectURL(url);
      },
    });
  };

  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const handleImportFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const data = JSON.parse(reader.result as string) as VaultExport;
        if (!data.version || !data.documents) {
          alert("Invalid vault export file format.");
          return;
        }
        if (!confirm(`Import ${data.documents.length} documents? Existing docs with matching slugs will be updated.`)) {
          return;
        }
        importMutation.mutate(data, {
          onSuccess: (result) => {
            alert(`Imported ${result.imported} documents.${result.errors.length > 0 ? `\n\nErrors:\n${result.errors.join("\n")}` : ""}`);
          },
        });
      } catch {
        alert("Failed to parse JSON file.");
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  };

  const [sidebarOpen, setSidebarOpen] = useState(true);

  // On mobile, close sidebar when a document is selected
  const handleSelect = (id: string) => {
    setSelectedId(id);
    if (window.innerWidth < 768) setSidebarOpen(false);
  };

  const roots = documents?.filter((d) => !d.parent_id) ?? [];
  const sortDocs = (a: KbDocument, b: KbDocument) => {
    if (a.is_folder !== b.is_folder) return a.is_folder ? -1 : 1;
    if (a.sort_order !== b.sort_order) return a.sort_order - b.sort_order;
    return a.title.localeCompare(b.title);
  };

  const sidebar = (
    <div className={`${sidebarOpen ? "block" : "hidden"} md:block w-full md:w-72 bg-white rounded-2xl shadow-sm p-4 overflow-y-auto shrink-0`}>
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-medium text-gray-500">Documents</h3>
        <div className="flex items-center gap-1">
          <button
            onClick={handleImportClick}
            title="Import"
            className="w-7 h-7 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors"
          >
            <Upload size={14} />
          </button>
          <button
            onClick={handleExport}
            title="Export"
            disabled={exportMutation.isPending}
            className="w-7 h-7 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors disabled:opacity-50"
          >
            <Download size={14} />
          </button>
          <button
            onClick={() => handleCreateFolder()}
            title="New folder"
            className="w-7 h-7 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors"
          >
            <FolderPlus size={14} />
          </button>
          <button
            onClick={() => handleCreate()}
            title="New document"
            className="w-7 h-7 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors"
          >
            <Plus size={14} />
          </button>
        </div>
      </div>

      <input type="file" ref={fileInputRef} onChange={handleImportFile} accept=".json" className="hidden" />

      {isLoading && <p className="text-sm text-gray-400">Loading...</p>}

      {[...roots].sort(sortDocs).map((doc) => (
        <DocTreeItem
          key={doc.id}
          doc={doc}
          documents={documents ?? []}
          selectedId={selectedId}
          onSelect={handleSelect}
          onDelete={handleDelete}
          onCreate={handleCreate}
          onCreateFolder={handleCreateFolder}
          sortDocs={sortDocs}
          depth={0}
        />
      ))}

      {roots.length === 0 && !isLoading && (
        <p className="text-sm text-gray-400">No documents yet</p>
      )}
    </div>
  );

  const editor = (
    <div className={`${!sidebarOpen ? "block" : "hidden md:block"} flex-1 bg-white rounded-2xl shadow-sm p-4 md:p-6 overflow-y-auto`}>
      {selectedDoc ? (
        selectedDoc.is_folder ? (
          <FolderView
            doc={selectedDoc}
            documents={documents ?? []}
            onSelect={handleSelect}
            onCreate={handleCreate}
            onCreateFolder={handleCreateFolder}
          />
        ) : (
          <div>
            <div className="flex items-center justify-between mb-6 gap-2">
              <div className="flex items-center gap-2 min-w-0 flex-1">
                <button
                  onClick={() => setSidebarOpen(true)}
                  className="md:hidden w-8 h-8 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors shrink-0"
                >
                  <ArrowLeft size={14} />
                </button>
                {editing ? (
                  <input
                    value={editTitle}
                    onChange={(e) => setEditTitle(e.target.value)}
                    className="text-xl font-bold text-gray-900 bg-gray-100 rounded-xl px-3 py-1 outline-none min-w-0 flex-1"
                  />
                ) : (
                  <h2 className="text-xl font-bold text-gray-900 truncate">{selectedDoc.title}</h2>
                )}
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {editing ? (
                  <button
                    onClick={handleSave}
                    className="px-4 py-2 bg-gray-900 text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors"
                  >
                    Save
                  </button>
                ) : (
                  <button
                    onClick={() => setEditing(true)}
                    className="w-8 h-8 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors"
                  >
                    <Edit2 size={14} />
                  </button>
                )}
                <button
                  onClick={() => setEditing(!editing)}
                  className="w-8 h-8 rounded-lg bg-gray-100 flex items-center justify-center text-gray-500 hover:bg-gray-200 transition-colors"
                >
                  <Eye size={14} />
                </button>
              </div>
            </div>

            {editing ? (
              <textarea
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                className="w-full h-[calc(100%-4rem)] min-h-[300px] md:min-h-[400px] bg-gray-50 rounded-xl p-4 text-sm font-mono text-gray-700 outline-none resize-none"
                placeholder="Write markdown here..."
              />
            ) : (
              <div className="prose prose-sm max-w-none text-gray-700">
                {selectedDoc.content ? (
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{selectedDoc.content}</ReactMarkdown>
                ) : (
                  <p className="text-gray-400">Empty document</p>
                )}
              </div>
            )}
          </div>
        )
      ) : (
        <div className="flex flex-col items-center justify-center h-full text-gray-400 text-sm gap-2">
          <button
            onClick={() => setSidebarOpen(true)}
            className="md:hidden px-4 py-2 bg-gray-100 rounded-xl text-sm text-gray-600 hover:bg-gray-200 transition-colors"
          >
            Browse Documents
          </button>
          <span className="hidden md:block">Select a document or create a new one</span>
        </div>
      )}
    </div>
  );

  return (
    <div className="flex flex-col md:flex-row gap-4 md:gap-6 h-full">
      {sidebar}
      {editor}
    </div>
  );
}

function FolderView({
  doc,
  documents,
  onSelect,
  onCreate,
  onCreateFolder,
}: {
  doc: KbDocument;
  documents: KbDocument[];
  onSelect: (id: string) => void;
  onCreate: (parentId?: string) => void;
  onCreateFolder: (parentId?: string) => void;
}) {
  const children = documents.filter((d) => d.parent_id === doc.id);
  const folders = children.filter((d) => d.is_folder);
  const docs = children.filter((d) => !d.is_folder);

  return (
    <div>
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between mb-6 gap-3">
        <div className="flex items-center gap-3">
          <Folder size={24} className="text-gray-400" />
          <h2 className="text-xl font-bold text-gray-900">{doc.title}</h2>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => onCreateFolder(doc.id)}
            className="px-3 py-2 bg-gray-100 text-gray-600 rounded-xl text-sm font-medium hover:bg-gray-200 transition-colors flex items-center gap-1"
          >
            <FolderPlus size={14} /> Subfolder
          </button>
          <button
            onClick={() => onCreate(doc.id)}
            className="px-3 py-2 bg-gray-900 text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors flex items-center gap-1"
          >
            <Plus size={14} /> New page
          </button>
        </div>
      </div>

      {children.length === 0 && (
        <p className="text-sm text-gray-400 text-center py-12">
          This folder is empty. Create a page or subfolder.
        </p>
      )}

      {folders.length > 0 && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-4">
          {folders.map((f) => (
            <button
              key={f.id}
              onClick={() => onSelect(f.id)}
              className="flex items-center gap-3 p-4 bg-gray-50 rounded-xl hover:bg-gray-100 transition-colors text-left"
            >
              <Folder size={20} className="text-amber-500 shrink-0" />
              <span className="text-sm font-medium text-gray-700 truncate">{f.title}</span>
            </button>
          ))}
        </div>
      )}

      {docs.length > 0 && (
        <div className="space-y-1">
          {docs.map((d) => (
            <button
              key={d.id}
              onClick={() => onSelect(d.id)}
              className="flex items-center gap-3 w-full p-3 rounded-xl hover:bg-gray-50 transition-colors text-left"
            >
              <FileText size={16} className="text-gray-400 shrink-0" />
              <div className="min-w-0 flex-1">
                <span className="text-sm text-gray-700 block truncate">{d.title}</span>
                <span className="text-xs text-gray-400">
                  {new Date(d.updated_at).toLocaleDateString()}
                </span>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function DocTreeItem({
  doc,
  documents,
  selectedId,
  onSelect,
  onDelete,
  onCreate,
  onCreateFolder,
  sortDocs,
  depth,
}: {
  doc: KbDocument;
  documents: KbDocument[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onCreate: (parentId?: string) => void;
  onCreateFolder: (parentId?: string) => void;
  sortDocs: (a: KbDocument, b: KbDocument) => number;
  depth: number;
}) {
  const children = documents.filter((d) => d.parent_id === doc.id);
  const [expanded, setExpanded] = useState(true);

  const isFolder = doc.is_folder;
  const hasChildren = children.length > 0;
  const Icon = isFolder ? Folder : FileText;

  return (
    <div>
      <div
        className={`flex items-center gap-1 p-2 rounded-lg cursor-pointer group ${
          selectedId === doc.id ? "bg-gray-100" : "hover:bg-gray-50"
        }`}
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
        onClick={() => {
          onSelect(doc.id);
          if (isFolder) setExpanded(!expanded);
        }}
      >
        {isFolder && (
          <span className="shrink-0 text-gray-400">
            {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          </span>
        )}
        {!isFolder && hasChildren && <span className="w-3" />}
        {!isFolder && !hasChildren && <span className="w-3" />}
        <Icon size={14} className={`shrink-0 ${isFolder ? "text-amber-500" : "text-gray-400"}`} />
        <span className="text-sm text-gray-700 truncate flex-1">{doc.title}</span>

        <div className="opacity-0 group-hover:opacity-100 flex items-center gap-0.5 transition-all">
          {isFolder && (
            <>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onCreate(doc.id);
                }}
                className="text-gray-400 hover:text-gray-600 p-0.5"
                title="New page here"
              >
                <Plus size={11} />
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onCreateFolder(doc.id);
                }}
                className="text-gray-400 hover:text-gray-600 p-0.5"
                title="New subfolder"
              >
                <FolderPlus size={11} />
              </button>
            </>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDelete(doc.id);
            }}
            className="text-gray-400 hover:text-red-500 p-0.5"
          >
            <Trash2 size={11} />
          </button>
        </div>
      </div>

      {isFolder && expanded && hasChildren && (
        <div>
          {[...children].sort(sortDocs).map((child) => (
            <DocTreeItem
              key={child.id}
              doc={child}
              documents={documents}
              selectedId={selectedId}
              onSelect={onSelect}
              onDelete={onDelete}
              onCreate={onCreate}
              onCreateFolder={onCreateFolder}
              sortDocs={sortDocs}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}
