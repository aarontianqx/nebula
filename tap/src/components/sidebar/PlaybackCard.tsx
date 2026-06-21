import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";

export function PlaybackCard() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const run = useDocumentStore((s) => s.run);
  const editable = useDocumentStore((s) => s.editable);
  const doc = useDocumentStore.getState;

  const repeatText = run.repeat === "Forever" ? "" : String(run.repeat.Times);
  const countdownSecs = Math.floor(run.start_delay_ms / 1000);
  const disabled = !isIdle || !editable;

  return (
    <>
      <h3>Playback</h3>
      <div className="card">
        <div className="field">
          <label className="label">Speed</label>
          <select
            value={run.speed}
            onChange={(e) => doc().setSpeed(parseFloat(e.target.value))}
            disabled={disabled}
            className="input"
          >
            <option value="0.5">0.5x</option>
            <option value="1">1x</option>
            <option value="2">2x</option>
            <option value="4">4x</option>
          </select>
        </div>
        <div className="field">
          <label className="label">Repeat</label>
          <input
            type="text"
            value={repeatText}
            onChange={(e) => doc().setRepeatText(e.target.value)}
            disabled={disabled}
            className="input"
            placeholder="empty = forever"
          />
        </div>
        <div className="field">
          <label className="label">Countdown</label>
          <div className="input-suffix">
            <input
              type="number"
              value={countdownSecs}
              onChange={(e) => doc().setCountdownSecs(parseInt(e.target.value, 10) || 0)}
              disabled={disabled}
              className="input"
              min={0}
            />
            <span>sec</span>
          </div>
        </div>
      </div>
    </>
  );
}
