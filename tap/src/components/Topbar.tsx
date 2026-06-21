import { formatDuration } from "../lib/actions";
import { useEngineStore } from "../stores/engineStore";
import { useRecorderStore } from "../stores/recorderStore";
import { useUiStore } from "../stores/uiStore";

export function Topbar() {
  const mode = useUiStore((s) => s.mode);
  const setMode = useUiStore((s) => s.setMode);
  const dryRun = useUiStore((s) => s.dryRun);
  const engineState = useEngineStore((s) => s.engineState);
  const mousePos = useEngineStore((s) => s.mousePos);
  const recorderState = useRecorderStore((s) => s.recorderState);
  const eventCount = useRecorderStore((s) => s.eventCount);
  const durationMs = useRecorderStore((s) => s.durationMs);

  const isIdle = engineState === "Idle";
  const isRecording = recorderState !== "Idle";
  const lockTabs = !isIdle || isRecording;

  return (
    <header className="topbar">
      <div className="brand">
        <div className="logo">tap</div>
        <div className="subtitle">Timed Action Performer</div>
      </div>
      <div className="topbar-tabs">
        <button className={`tab ${mode === "simple" ? "active" : ""}`} onClick={() => setMode("simple")} disabled={lockTabs}>
          Simple
        </button>
        <button
          className={`tab ${mode === "timeline" ? "active" : ""}`}
          onClick={() => setMode("timeline")}
          disabled={lockTabs}
        >
          Timeline
        </button>
      </div>
      <div className="topbar-actions">
        {dryRun && <span className="dry-run-badge">DRY RUN</span>}
        {mousePos && (
          <span className="mouse-pos">
            ({mousePos.x}, {mousePos.y})
          </span>
        )}
        {recorderState === "Recording" && (
          <span className="recording-badge">
            REC {formatDuration(durationMs)} | {eventCount} events
          </span>
        )}
        <span className={`state-badge state-${engineState.toLowerCase()}`}>{engineState}</span>
      </div>
    </header>
  );
}
