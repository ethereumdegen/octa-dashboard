import { useState } from "react";
import KbListView from "./components/KbListView";
import KbDetailView from "./components/KbDetailView";

export default function App() {
  const [selectedKbId, setSelectedKbId] = useState<string | null>(null);

  return (
    <div className="h-full overflow-y-auto text-gray-900">
      {selectedKbId ? (
        <KbDetailView kbId={selectedKbId} onBack={() => setSelectedKbId(null)} />
      ) : (
        <KbListView onSelect={setSelectedKbId} />
      )}
    </div>
  );
}
