import { MAX_HUMANIZE_DELAY_MS, MAX_HUMANIZE_POSITION_PX, repeatText as formatRepeat } from "../../lib/document";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";

export function PlaybackCard() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const run = useDocumentStore((s) => s.doc.run);
  const doc = useDocumentStore.getState;

  const repeatText = formatRepeat(run);
  const countdownSecs = Math.floor(run.start_delay_ms / 1000);
  const disabled = !isIdle;

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
        <div className="field">
          <label className="label">Position jitter</label>
          <div className="input-suffix">
            <span>±</span>
            <input
              type="number"
              value={run.humanize?.position_px ?? 0}
              onChange={(e) => doc().setHumanize({ position_px: parseInt(e.target.value, 10) || 0 })}
              disabled={disabled}
              className="input"
              min={0}
              max={MAX_HUMANIZE_POSITION_PX}
            />
            <span>px</span>
          </div>
        </div>
        <div className="field">
          <label className="label">Delay jitter</label>
          <div className="input-suffix">
            <span>±</span>
            <input
              type="number"
              value={run.humanize?.delay_ms ?? 0}
              onChange={(e) => doc().setHumanize({ delay_ms: parseInt(e.target.value, 10) || 0 })}
              disabled={disabled}
              className="input"
              min={0}
              max={MAX_HUMANIZE_DELAY_MS}
              step={10}
            />
            <span>ms</span>
          </div>
          <div className="hint">Varies where clicks land and when steps start, so runs are not identical. 0 = exact.</div>
        </div>
      </div>
    </>
  );
}
