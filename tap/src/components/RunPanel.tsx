import { startTimelineRun } from "../lib/run";
import { useDocumentStore } from "../stores/documentStore";
import { useEngineStore } from "../stores/engineStore";
import { useRecorderStore } from "../stores/recorderStore";
import { useToolStore } from "../stores/toolStore";
import { useUiStore } from "../stores/uiStore";

export function RunPanel() {
  const mode = useUiStore((s) => s.mode);

  const engineState = useEngineStore((s) => s.engineState);
  const countdown = useEngineStore((s) => s.countdown);
  const executedCount = useEngineStore((s) => s.executedCount);
  const iteration = useEngineStore((s) => s.iteration);
  const lastAction = useEngineStore((s) => s.lastAction);
  const engine = useEngineStore;

  const recorderState = useRecorderStore((s) => s.recorderState);
  const recorder = useRecorderStore;

  const actionType = useToolStore((s) => s.actionType);
  const keyClickRunning = useToolStore((s) => s.keyClickRunning);
  const keyClickCount = useToolStore((s) => s.keyClickCount);
  const tool = useToolStore;

  const timelineLength = useDocumentStore((s) => s.timeline.length);
  const variableCount = useDocumentStore((s) => s.variables.length);

  const isIdle = engineState === "Idle";
  const isRunning = engineState === "Running";
  const isPaused = engineState === "Paused";
  const isArming = engineState === "Arming";
  const isRecording = recorderState === "Recording";
  const isRecordingPaused = recorderState === "Paused";
  const canRecord = isIdle && recorderState === "Idle";

  const showPlay =
    recorderState === "Idle" && isIdle && !keyClickRunning && (mode === "timeline" || actionType !== "key-to-click");
  const showKeyClickStart =
    recorderState === "Idle" && isIdle && !keyClickRunning && mode === "simple" && actionType === "key-to-click";

  async function handlePlay() {
    if (mode === "simple") {
      await tool.getState().startSimple();
      return;
    }
    // Parameterized macros: collect run-time values first, then the dialog starts.
    if (variableCount > 0) {
      useUiStore.getState().openVariableDialog("run");
      return;
    }
    await startTimelineRun();
  }

  return (
    <>
      <h3>Controls</h3>
      <div className="card controls-card">
        <div className="control-buttons">
          {mode === "timeline" && canRecord && (
            <button className="btn btn-record" onClick={() => recorder.getState().start()}>
              Record
            </button>
          )}
          {isRecording && (
            <>
              <button className="btn" onClick={() => recorder.getState().pause()}>
                Pause
              </button>
              <button className="btn btn-danger" onClick={() => recorder.getState().stop()}>
                Stop
              </button>
            </>
          )}
          {isRecordingPaused && (
            <>
              <button className="btn btn-record" onClick={() => recorder.getState().resume()}>
                Resume
              </button>
              <button className="btn btn-danger" onClick={() => recorder.getState().stop()}>
                Stop
              </button>
            </>
          )}
          {showPlay && (
            <button
              className="btn btn-primary btn-large"
              onClick={handlePlay}
              disabled={mode === "timeline" && timelineLength === 0}
            >
              Play
            </button>
          )}
          {showKeyClickStart && (
            <button className="btn btn-primary btn-large" onClick={() => tool.getState().startKeyClick()}>
              Start Key-&gt;Click
            </button>
          )}
          {keyClickRunning && (
            <button className="btn btn-danger btn-large" onClick={() => tool.getState().stopKeyClick()}>
              Stop Key-&gt;Click
            </button>
          )}
          {isArming && (
            <div className="countdown-display">
              <span className="countdown-number">{countdown}</span>
              <span className="countdown-label">Starting in...</span>
            </div>
          )}
          {isRunning && (
            <>
              <button className="btn" onClick={() => engine.getState().pause()}>
                Pause
              </button>
              <button className="btn btn-danger" onClick={() => engine.getState().stop()}>
                Stop
              </button>
            </>
          )}
          {isPaused && (
            <>
              <button className="btn btn-primary" onClick={() => engine.getState().resume()}>
                Resume
              </button>
              <button className="btn btn-danger" onClick={() => engine.getState().stop()}>
                Stop
              </button>
            </>
          )}
        </div>

        {(isRunning || isPaused || isArming) && (
          <div className="stats">
            <div className="stat">
              <span className="stat-value">{executedCount}</span>
              <span className="stat-label">Actions</span>
            </div>
            <div className="stat">
              <span className="stat-value">{iteration}</span>
              <span className="stat-label">Iterations</span>
            </div>
          </div>
        )}

        {keyClickRunning && (
          <div className="stats">
            <div className="stat">
              <span className="stat-value">{keyClickCount}</span>
              <span className="stat-label">Clicks</span>
            </div>
          </div>
        )}

        {lastAction && (isRunning || isPaused) && (
          <div className="last-action">
            <span className="last-action-label">Last:</span>
            <span className="last-action-value">{lastAction}</span>
          </div>
        )}

        <button
          className="btn btn-emergency"
          onClick={() => engine.getState().emergencyStop()}
          disabled={isIdle && recorderState === "Idle" && !keyClickRunning}
        >
          Emergency Stop
        </button>
      </div>
    </>
  );
}
