import { useEffect } from "react";

import { ActivityLog } from "./components/ActivityLog";
import { RunPanel } from "./components/RunPanel";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Topbar } from "./components/Topbar";
import { TimelineEditor } from "./components/timeline/TimelineEditor";
import { VariableDialog } from "./components/modals/VariableDialog";
import { setupEventListeners } from "./lib/events";
import { useDocumentStore } from "./stores/documentStore";
import { useUiStore } from "./stores/uiStore";

export default function App() {
  const mode = useUiStore((s) => s.mode);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let cancelled = false;
    setupEventListeners().then((c) => {
      if (cancelled) c();
      else cleanup = c;
    });
    // Seed the editor from the backend's canonical document.
    void useDocumentStore.getState().loadProfiles();
    void useDocumentStore.getState().refreshFromBackend();
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, []);

  return (
    <div className="app">
      <Topbar />
      <div className="layout">
        <Sidebar />
        <main className="main">
          <RunPanel />
          {mode === "timeline" && <TimelineEditor />}
          <ActivityLog />
        </main>
      </div>
      <VariableDialog />
      <StatusBar />
    </div>
  );
}
