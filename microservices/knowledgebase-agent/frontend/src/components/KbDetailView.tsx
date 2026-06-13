import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, Book, Loader2, FileText, MessageSquare, Settings } from "lucide-react";
import { kbApi } from "../api/kb";
import DocumentsTab from "./DocumentsTab";
import ChatTab from "./ChatTab";
import SettingsTab from "./SettingsTab";

type Tab = "documents" | "chat" | "settings";

const TABS: { id: Tab; label: string; icon: typeof FileText }[] = [
  { id: "documents", label: "Documents", icon: FileText },
  { id: "chat", label: "Chat", icon: MessageSquare },
  { id: "settings", label: "Settings", icon: Settings },
];

export default function KbDetailView({
  kbId,
  onBack,
}: {
  kbId: string;
  onBack: () => void;
}) {
  const [tab, setTab] = useState<Tab>("documents");
  const { data: kb, isLoading } = useQuery({
    queryKey: ["kb", kbId],
    queryFn: () => kbApi.getKb(kbId),
  });

  if (isLoading || !kb) {
    return (
      <div className="flex justify-center py-16 text-gray-400">
        <Loader2 className="animate-spin" />
      </div>
    );
  }

  return (
    <div className="mx-auto flex h-full max-w-5xl flex-col">
      <button
        onClick={onBack}
        className="mb-4 inline-flex shrink-0 items-center gap-1.5 text-sm text-gray-500 hover:text-gray-900"
      >
        <ArrowLeft size={15} /> All knowledgebases
      </button>

      <div className="mb-5 flex shrink-0 items-center gap-3">
        <span
          className="flex h-11 w-11 items-center justify-center rounded-xl text-white"
          style={{ background: kb.accent_color || "#111827" }}
        >
          <Book size={22} />
        </span>
        <div>
          <h1 className="text-xl font-bold">{kb.name}</h1>
          {kb.description && <p className="text-sm text-gray-500">{kb.description}</p>}
        </div>
      </div>

      <div className="mb-5 flex shrink-0 gap-1 border-b border-gray-200">
        {TABS.map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`inline-flex items-center gap-1.5 border-b-2 px-4 py-2.5 text-sm font-medium transition ${
                tab === t.id
                  ? "border-gray-900 text-gray-900"
                  : "border-transparent text-gray-500 hover:text-gray-800"
              }`}
            >
              <Icon size={15} /> {t.label}
            </button>
          );
        })}
      </div>

      <div className={`min-h-0 flex-1 ${tab === "chat" ? "" : "overflow-y-auto"}`}>
        {tab === "documents" && <DocumentsTab kbId={kbId} />}
        {tab === "chat" && <ChatTab kbId={kbId} />}
        {tab === "settings" && <SettingsTab kb={kb} onDeleted={onBack} />}
      </div>
    </div>
  );
}
