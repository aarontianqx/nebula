import { useEngineStore } from "../stores/engineStore";

export function StatusBar() {
  const engineState = useEngineStore((s) => s.engineState);
  const status = useEngineStore((s) => s.status);
  const uiMessage = useEngineStore((s) => s.uiMessage);
  const executedCount = useEngineStore((s) => s.executedCount);
  const iteration = useEngineStore((s) => s.iteration);

  const active = engineState === "Running" || engineState === "Paused";

  return (
    <footer className="statusbar">
      <span className={`status-state state-${engineState.toLowerCase()}`}>{engineState}</span>
      <span className="status-divider">|</span>
      <span className="status-value">{uiMessage || status}</span>
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
