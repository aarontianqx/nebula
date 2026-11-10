import { useEffect, useMemo, useState } from "react";

import { variableEntries } from "../../lib/document";
import { api } from "../../lib/ipc";
import { startTimelineRun } from "../../lib/run";
import { useDocumentStore } from "../../stores/documentStore";
import { useUiStore } from "../../stores/uiStore";
import { log } from "../../stores/logStore";

export function VariableDialog() {
  const show = useUiStore((s) => s.showVariableDialog);
  const dialogMode = useUiStore((s) => s.variableDialogMode);
  const doc = useDocumentStore((s) => s.doc);
  const close = () => useUiStore.getState().setShowVariableDialog(false);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const variables = useMemo(() => variableEntries(doc), [doc]);

  useEffect(() => {
    if (!show) return;
    const seed: Record<string, unknown> = {};
    for (const [name, def] of variables) {
      seed[name] = def.default ?? (def.type === "number" ? 0 : def.type === "boolean" ? false : "");
    }
    setValues(seed);
  }, [show, variables]);

  if (!show) return null;

  async function handleApply() {
    try {
      await api.setRuntimeVariables(values);
      log("Variables applied");
      close();
      if (dialogMode === "run") {
        await startTimelineRun();
      }
    } catch (err) {
      log(`Failed to apply variables: ${String(err)}`);
    }
  }

  return (
    <div className="modal-overlay" onClick={close}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>{dialogMode === "run" ? "Set Variables & Run" : "Set Variables"}</h3>
        <div className="variable-form">
          {variables.map(([name, def]) => (
            <div key={name} className="field">
              <label className="label">
                {name}
                {def.description && <span className="var-desc"> - {def.description}</span>}
              </label>
              {def.type === "boolean" ? (
                <label className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={!!values[name]}
                    onChange={(e) => setValues((prev) => ({ ...prev, [name]: e.target.checked }))}
                  />
                  Enabled
                </label>
              ) : def.type === "number" ? (
                <input
                  type="number"
                  value={(values[name] as number) ?? 0}
                  onChange={(e) => setValues((prev) => ({ ...prev, [name]: parseFloat(e.target.value) || 0 }))}
                  className="input"
                />
              ) : (
                <input
                  type="text"
                  value={(values[name] as string) ?? ""}
                  onChange={(e) => setValues((prev) => ({ ...prev, [name]: e.target.value }))}
                  className="input"
                />
              )}
            </div>
          ))}
        </div>
        <div className="modal-actions">
          <button className="btn" onClick={close}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={handleApply}>
            {dialogMode === "run" ? "Apply & Run" : "Apply"}
          </button>
        </div>
      </div>
    </div>
  );
}
