import { useEngineStore } from "../stores/engineStore";
import { useToolStore } from "../stores/toolStore";

export function StatusBar() {
  const engineState = useEngineStore((s) => s.engineState);
  const status = useEngineStore((s) => s.status);
  const uiMessage = useEngineStore((s) => s.uiMessage);
  const executedCount = useEngineStore((s) => s.executedCount);
  const iteration = useEngineStore((s) => s.iteration);
  const keyClickRunning = useToolStore((s) => s.keyClickRunning);

  const active = engineState === "Running" || engineState === "Paused";

  return (
    <footer className="statusbar">
      <span className={`status-state state-${engineState.toLowerCase()}`}>{engineState}</span>
      <span className="status-divider">|</span>
      <span className="status-value">{uiMessage || status}</span>
      {keyClickRunning && (
        <>
          <span className="status-divider">|</span>
          <span className="status-keyclick">Key-&gt;Click: hold A-Z to click, Space to stop</span>
        </>
      )}
      {active && (
        <>
          <span className="status-divider">|</span>
          <span className="status-stats">
            {executedCount} actions, {iteration} iters
          </span>
        </>
      )}
    </footer>
  );
}
