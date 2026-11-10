import { ACTION_GROUPS, ACTION_LABELS, defaultActionForKind } from "../../lib/actions";
import { countVariables } from "../../lib/document";
import { api } from "../../lib/ipc";
import type { DslActionKind } from "../../lib/types";
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

/** Append a new step of `kind` and open it in the Inspector. */
function addAction(kind: DslActionKind): void {
  const store = useDocumentStore.getState();
  const index = store.doc.timeline.length;
  store.appendAction(defaultActionForKind(kind));
  useUiStore.getState().selectAction(index);
}

export function TimelineEditor() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const count = useDocumentStore((s) => s.doc.timeline.length);
  const issues = useDocumentStore((s) => s.issues);
  const variableCount = useDocumentStore((s) => countVariables(s.doc));
  const view = useUiStore((s) => s.timelineView);
  const ui = useUiStore.getState;

  const canEdit = isIdle;

  async function gotoCode() {
    ui().setTimelineView("code");
    try {
      // The YAML is rendered from the backend's document: make sure it has the latest edits.
      await useDocumentStore.getState().flush();
      ui().setYamlContent(await api.exportYaml());
    } catch (err) {
      log(`Export failed: ${String(err)}`);
    }
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
          <select
            className="btn btn-sm add-action"
            value=""
            onChange={(e) => e.target.value && addAction(e.target.value as DslActionKind)}
            disabled={!canEdit}
            title="Add an action to the end of the timeline"
            aria-label="Add action"
          >
            <option value="" disabled>
              + Add
            </option>
            {ACTION_GROUPS.map((group) => (
              <optgroup key={group.label} label={group.label}>
                {group.kinds.map((kind) => (
                  <option key={kind} value={kind}>
                    {ACTION_LABELS[kind]}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
          <button className="btn btn-sm" onClick={() => useDocumentStore.getState().batchAdjustDelay(-50)} disabled={!canEdit} title="Shift all -50ms">
            All -50
          </button>
          <button className="btn btn-sm" onClick={() => useDocumentStore.getState().batchAdjustDelay(50)} disabled={!canEdit} title="Shift all +50ms">
            All +50
          </button>
          <button
            className="btn btn-sm"
            onClick={() => useDocumentStore.getState().exportToFile()}
            disabled={!isIdle}
            title="Export to a YAML file"
          >
            Export
          </button>
          <button
            className="btn btn-sm"
            onClick={() => useDocumentStore.getState().importFromFile()}
            disabled={!isIdle}
            title="Import from a YAML file"
          >
            Import
          </button>
          <button
            className="btn btn-sm"
            onClick={() => useUiStore.getState().setShowVariableDefinitions(true)}
            disabled={!isIdle}
            title="Define the variables this macro uses"
          >
            Variables
          </button>
          {variableCount > 0 && (
            <button className="btn btn-sm" onClick={() => ui().openVariableDialog("edit")} disabled={!isIdle} title="Set variable values for the next run">
              Values
            </button>
          )}
        </div>
      </div>

      {issues.length > 0 && view !== "code" && (
        <div className="warning-box timeline-issues">
          {issues.map((issue, idx) => (
            <div key={idx}>
              <strong>{issue.path || "macro"}</strong>: {issue.message}
            </div>
          ))}
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
