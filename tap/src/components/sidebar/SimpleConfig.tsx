import type { SimpleActionType } from "../../lib/types";
import { useEngineStore } from "../../stores/engineStore";
import { useKeyClickStore } from "../../stores/keyClickStore";
import { usePickStore } from "../../stores/pickStore";
import { SIMPLE_KEY_FIELD, useSimpleStore } from "../../stores/simpleStore";
import { KeyClickFields } from "./KeyClickFields";
import { MouseButtonSelect, NumberWithUnit, PositionFields } from "./SimpleFormFields";

export function SimpleConfig() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const keyClickRunning = useKeyClickStore((s) => s.running);
  const simple = useSimpleStore();
  const capturingKey = usePickStore((s) => s.captureTarget === SIMPLE_KEY_FIELD);

  return (
    <>
      <h3>Configuration</h3>
      <div className="card">
        <div className="field">
          <label className="label">Action</label>
          <select
            value={simple.actionType}
            onChange={(e) => simple.setActionType(e.target.value as SimpleActionType)}
            disabled={!isIdle || keyClickRunning}
            className="input"
          >
            <option value="click">Click</option>
            <option value="key">Key Press</option>
            <option value="key-to-click">Key -&gt; Click</option>
          </select>
        </div>

        {simple.actionType === "click" && (
          <>
            <MouseButtonSelect value={simple.clickButton} disabled={!isIdle} onChange={simple.setClickButton} />
            <PositionFields
              x={simple.clickX}
              y={simple.clickY}
              disabled={!isIdle}
              onXChange={simple.setClickX}
              onYChange={simple.setClickY}
              onPick={simple.openPicker}
            />
          </>
        )}

        {simple.actionType === "key" && (
          <div className="field">
            <label className="label">Key</label>
            <div className="input-with-button">
              <input
                type="text"
                value={simple.keyName}
                onChange={(e) => simple.setKeyName(e.target.value)}
                disabled={!isIdle || capturingKey}
                className="input"
                placeholder="e.g., Space, Enter"
              />
              <button
                className="btn btn-pick"
                onClick={simple.captureKeyName}
                disabled={!isIdle || capturingKey}
                title="Press the next key to capture it"
              >
                {capturingKey ? "Press..." : "Capture"}
              </button>
            </div>
          </div>
        )}

        {simple.actionType === "key-to-click" ? (
          <KeyClickFields />
        ) : (
          <>
            <NumberWithUnit
              label="Interval"
              unit="ms"
              value={simple.intervalMs}
              fallback={100}
              min={50}
              disabled={!isIdle}
              onChange={simple.setIntervalMs}
            />
            <div className="field">
              <label className="label">Repeat</label>
              <input
                type="text"
                value={simple.repeatText}
                onChange={(e) => simple.setRepeatText(e.target.value)}
                disabled={!isIdle}
                className="input"
                placeholder="empty = forever"
              />
            </div>
            <NumberWithUnit
              label="Countdown"
              unit="sec"
              value={simple.countdownSecs}
              fallback={0}
              min={0}
              disabled={!isIdle}
              onChange={simple.setCountdownSecs}
            />
          </>
        )}
      </div>
    </>
  );
}
