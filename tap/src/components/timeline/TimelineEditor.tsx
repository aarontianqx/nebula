import type { ChangeEvent } from "react";

import { defaultActionForKind } from "../../lib/actions";
import { api } from "../../lib/ipc";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";
import { CodeView } from "./CodeView";
import { Inspector } from "./Inspector";
import { TimelineListView } from "./TimelineListView";
import { TimelineRailView } from "./TimelineRailView";

function log(msg: string): void {
  useEngineStore.getState().addLog(msg);
}

export function TimelineEditor() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const count = useDocumentStore((s) => s.timeline.length);
  const editable = useDocumentStore((s) => s.editable);
  const name = useDocumentStore((s) => s.name);
  const variableCount = useDocumentStore((s) => s.variables.length);
  const view = useUiStore((s) => s.timelineView);
  const ui = useUiStore.getState;

  const canEdit = isIdle && editable;

  async function gotoCode() {
    ui().setTimelineView("code");
    try {
      ui().setYamlContent(await api.exportYaml());
    } catch (err) {
      log(`Export failed: ${String(err)}`);
    }
  }

  async function handleDownload() {
    try {
      const yaml = await api.exportYaml();
      const blob = new Blob([yaml], { type: "text/yaml" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${name || "macro"}.yaml`;
      a.click();
      URL.revokeObjectURL(url);
      log(`Downloaded ${name}.yaml`);
    } catch (err) {
      log(`Download failed: ${String(err)}`);
    }
  }

  function handleFile(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      ui().setYamlContent((ev.target?.result as string) ?? "");
      ui().setTimelineView("code");
      log(`Loaded file: ${file.name}`);
    };
    reader.readAsText(file);
    e.target.value = "";
  }

  return (
    <>
      <div className="timeline-header">
        <h3>Timeline ({count} actions)</h3>
        <div className="timeline-view-tabs">
          <button
            className={`tab-btn ${view === "list" ? "active" : ""}`}
            onClick={() => ui().setTimelineView("list")}
            disabled={!isIdle}
          >
            List
          </button>
          <button
            className={`tab-btn ${view === "rail" ? "active" : ""}`}
            onClick={() => ui().setTimelineView("rail")}
            disabled={!isIdle}
          >
            Rail
          </button>
          <button className={`tab-btn ${view === "code" ? "active" : ""}`} onClick={gotoCode} disabled={!isIdle}>
            Code
          </button>
        </div>
        <div className="timeline-toolbar">
          <button className="btn btn-sm" onClick={() => useDocumentStore.getState().appendAction(defaultActionForKind("Click"))} disabled={!canEdit} title="Add action">
            + Add
          </button>
          <button className="btn btn-sm" onClick={() => useDocumentStore.getState().batchAdjustDelay(-50)} disabled={!canEdit} title="Shift all -50ms">
            All -50
          </button>
          <button className="btn btn-sm" onClick={() => useDocumentStore.getState().batchAdjustDelay(50)} disabled={!canEdit} title="Shift all +50ms">
            All +50
          </button>
          <button className="btn btn-sm" onClick={handleDownload} disabled={!isIdle} title="Download YAML">
            Export
          </button>
          <label className="btn btn-sm" style={{ cursor: isIdle ? "pointer" : "not-allowed" }}>
            Import
            <input type="file" accept=".yaml,.yml" onChange={handleFile} disabled={!isIdle} style={{ display: "none" }} />
          </label>
          {variableCount > 0 && (
            <button className="btn btn-sm" onClick={() => ui().setShowVariableDialog(true)} disabled={!isIdle} title="Set variables">
              Variables
            </button>
          )}
        </div>
      </div>

      {!editable && view !== "code" && (
        <div className="info-box readonly-banner">
          This macro uses variables/expressions. The visual view is a resolved preview; edit it in the{" "}
          <strong>Code (YAML)</strong> view to keep parameters intact.
        </div>
      )}

      {view === "code" ? (
        <CodeView />
      ) : (
        <div className="timeline-body">
          {view === "list" ? <TimelineListView /> : <TimelineRailView />}
          <Inspector />
        </div>
      )}
    </>
  );
}
