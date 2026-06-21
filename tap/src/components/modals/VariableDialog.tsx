import { useEffect, useState } from "react";

import { api } from "../../lib/ipc";
import { startTimelineRun } from "../../lib/run";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";

export function VariableDialog() {
  const show = useUiStore((s) => s.showVariableDialog);
  const dialogMode = useUiStore((s) => s.variableDialogMode);
  const variables = useDocumentStore((s) => s.variables);
  const close = () => useUiStore.getState().setShowVariableDialog(false);
  const [values, setValues] = useState<Record<string, unknown>>({});

  useEffect(() => {
    if (!show) return;
    const seed: Record<string, unknown> = {};
    for (const v of variables) {
      seed[v.name] = v.default ?? (v.var_type === "number" ? 0 : v.var_type === "boolean" ? false : "");
    }
    setValues(seed);
  }, [show, variables]);

  if (!show) return null;

  async function handleApply() {
    try {
      await api.setRuntimeVariables(values);
      useEngineStore.getState().addLog("Variables applied");
      close();
      if (dialogMode === "run") {
        await startTimelineRun();
      }
    } catch (err) {
      useEngineStore.getState().addLog(`Failed to apply variables: ${String(err)}`);
    }
  }

  return (
    <div className="modal-overlay" onClick={close}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>{dialogMode === "run" ? "Set Variables & Run" : "Set Variables"}</h3>
        <div className="variable-form">
          {variables.map((v) => (
            <div key={v.name} className="field">
              <label className="label">
                {v.name}
                {v.description && <span className="var-desc"> - {v.description}</span>}
              </label>
              {v.var_type === "boolean" ? (
                <label className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={!!values[v.name]}
                    onChange={(e) => setValues((prev) => ({ ...prev, [v.name]: e.target.checked }))}
                  />
                  Enabled
                </label>
              ) : v.var_type === "number" ? (
                <input
                  type="number"
                  value={(values[v.name] as number) ?? 0}
                  onChange={(e) => setValues((prev) => ({ ...prev, [v.name]: parseFloat(e.target.value) || 0 }))}
                  className="input"
                />
              ) : (
                <input
                  type="text"
                  value={(values[v.name] as string) ?? ""}
                  onChange={(e) => setValues((prev) => ({ ...prev, [v.name]: e.target.value }))}
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
