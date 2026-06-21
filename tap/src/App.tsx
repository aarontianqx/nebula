import { useEffect } from "react";

import { ActivityLog } from "./components/ActivityLog";
import { OnboardingBanner } from "./components/OnboardingBanner";
import { PermissionBanner } from "./components/PermissionBanner";
import { RunPanel } from "./components/RunPanel";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Topbar } from "./components/Topbar";
import { TimelineEditor } from "./components/timeline/TimelineEditor";
import { VariableDialog } from "./components/modals/VariableDialog";
import { setupEventListeners } from "./lib/events";
import { useDocumentStore } from "./stores/documentStore";
import { usePermissionStore } from "./stores/permissionStore";
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
    const doc = useDocumentStore.getState();
    void doc.loadProfiles();
    void doc.loadRecents();
    void doc.loadTemplates();
    void doc.refreshFromBackend();
    // Probe OS permissions now, and again when the window regains focus so
    // grants made in System Settings reflect without a manual re-check.
    void usePermissionStore.getState().refresh();
    void useUiStore.getState().loadDryRun();
    const onFocus = () => void usePermissionStore.getState().refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      cleanup?.();
      window.removeEventListener("focus", onFocus);
    };
  }, []);

  return (
    <div className="app">
      <Topbar />
      <div className="layout">
        <Sidebar />
        <main className="main">
          <OnboardingBanner />
          <PermissionBanner />
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
