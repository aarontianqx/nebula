import { useMemo } from "react";

import { ACTION_GROUPS } from "../../lib/actions";
import { availableReferences, issuesForStep } from "../../lib/document";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";
import { ActionParams, KindSelect } from "./ActionParams";
import { ReferencesContext } from "./fields";

/** `timeline[3].action.condition` reads as `condition` inside the Inspector of step 4. */
function relativePath(path: string): string {
  return path.replace(/^timeline\[\d+\]\.?(action\.?)?/, "");
}

export function Inspector() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const doc = useDocumentStore((s) => s.doc);
  const issues = useDocumentStore((s) => s.issues);
  const selectedIdx = useUiStore((s) => s.selectedActionIdx);

  const references = useMemo(() => availableReferences(doc), [doc]);
  const stepIssues = useMemo(() => (selectedIdx === null ? [] : issuesForStep(issues, selectedIdx)), [issues, selectedIdx]);

  const canEdit = isIdle;
  const item = selectedIdx !== null ? doc.timeline[selectedIdx] : undefined;

  if (selectedIdx === null || !item) {
    return (
      <div className="card inspector-card">
        <div className="inspector-empty">Select an action to edit its parameters.</div>
      </div>
    );
  }

  const idx = selectedIdx;
  const store = useDocumentStore.getState;

  return (
    <div className="card inspector-card">
      <div className="inspector-head">
        <h4>Inspector</h4>
        <span className="inspector-index">#{idx + 1}</span>
      </div>

      {/* Keyed by step so per-field editing state never carries over to another step. */}
      <ReferencesContext.Provider value={references}>
        <div key={idx}>
          <KindSelect
            action={item.action}
            groups={ACTION_GROUPS}
            label="Type"
            disabled={!canEdit}
            onChange={(action) => store().setAction(idx, action)}
          />

          <div className="field">
            <label className="label">At (ms)</label>
            <input
              type="number"
              value={item.at_ms}
              onChange={(e) => store().setAtMs(idx, parseInt(e.target.value, 10) || 0)}
              disabled={!canEdit}
              className="input"
              min={0}
            />
          </div>

          <ActionParams action={item.action} disabled={!canEdit} onChange={(action) => store().setAction(idx, action)} />

          <div className="field">
            <label className="label">Note</label>
            <input
              type="text"
              value={item.note ?? ""}
              onChange={(e) => store().setNote(idx, e.target.value)}
              disabled={!canEdit}
              className="input"
              placeholder="optional"
            />
          </div>
        </div>
      </ReferencesContext.Provider>

      {stepIssues.length > 0 && (
        <div className="warning-box inspector-issues">
          {stepIssues.map((issue, i) => (
            <div key={i}>
              {relativePath(issue.path) && <strong>{relativePath(issue.path)}: </strong>}
              {issue.message}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
