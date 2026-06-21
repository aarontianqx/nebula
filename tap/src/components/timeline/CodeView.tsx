import { api } from "../../lib/ipc";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";

export function CodeView() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const yamlContent = useUiStore((s) => s.yamlContent);
  const yamlErrors = useUiStore((s) => s.yamlErrors);
  const ui = useUiStore.getState;

  async function handleApply() {
    try {
      const errors = await api.validateYaml(yamlContent);
      if (errors && errors.length > 0) {
        ui().setYamlErrors(errors);
        useEngineStore.getState().addLog(`Validation errors: ${errors.length}`);
        return;
      }
      ui().setYamlErrors([]);
      await useDocumentStore.getState().importYaml(yamlContent);
      ui().setTimelineView("list");
    } catch (err) {
      useEngineStore.getState().addLog(`Import failed: ${String(err)}`);
    }
  }

  async function handleRefresh() {
    try {
      ui().setYamlContent(await api.exportYaml());
      ui().setYamlErrors([]);
    } catch (err) {
      useEngineStore.getState().addLog(`Export failed: ${String(err)}`);
    }
  }

  return (
    <div className="card code-card">
      <textarea
        className="code-editor"
        value={yamlContent}
        onChange={(e) => ui().setYamlContent(e.target.value)}
        disabled={!isIdle}
        placeholder="# YAML macro definition..."
        spellCheck={false}
      />
      {yamlErrors.length > 0 && (
        <div className="yaml-errors">
          {yamlErrors.map((err, idx) => (
            <div key={idx} className="yaml-error">
              {err.line != null && <span className="error-line">Line {err.line}:</span>}
              <span className="error-path">{err.path}</span>
              <span className="error-msg">{err.message}</span>
            </div>
          ))}
        </div>
      )}
      <div className="code-actions">
        <button className="btn btn-primary" onClick={handleApply} disabled={!isIdle || !yamlContent.trim()}>
          Apply Changes
        </button>
        <button className="btn" onClick={handleRefresh} disabled={!isIdle}>
          Refresh from Document
        </button>
      </div>
    </div>
  );
}
