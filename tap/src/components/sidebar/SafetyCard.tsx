import { useUiStore } from "../../stores/uiStore";

export function SafetyCard() {
  const dryRun = useUiStore((s) => s.dryRun);
  const setDryRun = useUiStore((s) => s.setDryRun);

  return (
    <>
      <h3>Safety</h3>
      <div className="card safety-card">
        <div className="safety-info">
          <span className="safety-icon">!</span>
          <div>
            <div className="safety-title">Emergency Stop</div>
            <div className="safety-key">Ctrl + Shift + Backspace</div>
          </div>
        </div>
        <label className="checkbox-label dry-run-toggle">
          <input type="checkbox" checked={dryRun} onChange={(e) => setDryRun(e.target.checked)} />
          <span>Dry run (preview without real clicks/keys)</span>
        </label>
      </div>
    </>
  );
}
