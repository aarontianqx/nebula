import type { KeyClickLocationMode, MouseButton, SimpleActionType } from "../../lib/types";
import { useEngineStore } from "../../stores/engineStore";
import { useToolStore } from "../../stores/toolStore";

export function SimpleConfig() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");

  const actionType = useToolStore((s) => s.actionType);
  const clickX = useToolStore((s) => s.clickX);
  const clickY = useToolStore((s) => s.clickY);
  const clickButton = useToolStore((s) => s.clickButton);
  const keyName = useToolStore((s) => s.keyName);
  const capturingKey = useToolStore((s) => s.capturingKey);
  const intervalMs = useToolStore((s) => s.intervalMs);
  const repeatText = useToolStore((s) => s.repeatText);
  const countdownSecs = useToolStore((s) => s.countdownSecs);
  const keyClickRunning = useToolStore((s) => s.keyClickRunning);
  const keyClickCount = useToolStore((s) => s.keyClickCount);
  const keyClickInterval = useToolStore((s) => s.keyClickInterval);
  const keyClickHoldDelay = useToolStore((s) => s.keyClickHoldDelay);
  const keyClickButton = useToolStore((s) => s.keyClickButton);
  const keyClickLocationMode = useToolStore((s) => s.keyClickLocationMode);
  const keyClickOnlyTargetFocused = useToolStore((s) => s.keyClickOnlyTargetFocused);
  const tool = useToolStore.getState;

  return (
    <>
      <h3>Configuration</h3>
      <div className="card">
        <div className="field">
          <label className="label">Action</label>
          <select
            value={actionType}
            onChange={(e) => tool().setActionType(e.target.value as SimpleActionType)}
            disabled={!isIdle || keyClickRunning}
            className="input"
          >
            <option value="click">Click</option>
            <option value="key">Key Press</option>
            <option value="key-to-click">Key -&gt; Click</option>
          </select>
        </div>

        {actionType === "click" && (
          <>
            <div className="field">
              <label className="label">Mouse Button</label>
              <select
                value={clickButton}
                onChange={(e) => tool().setClickButton(e.target.value as MouseButton)}
                disabled={!isIdle}
                className="input"
              >
                <option value="Left">Left</option>
                <option value="Right">Right</option>
                <option value="Middle">Middle</option>
              </select>
            </div>
            <div className="field">
              <label className="label">X</label>
              <input
                type="number"
                value={clickX}
                onChange={(e) => tool().setClickX(parseInt(e.target.value, 10) || 0)}
                disabled={!isIdle}
                className="input"
              />
            </div>
            <div className="field">
              <label className="label">Y</label>
              <div className="input-with-button">
                <input
                  type="number"
                  value={clickY}
                  onChange={(e) => tool().setClickY(parseInt(e.target.value, 10) || 0)}
                  disabled={!isIdle}
                  className="input"
                />
                <button className="btn btn-pick" onClick={() => tool().openPicker()} disabled={!isIdle}>
                  Pick
                </button>
              </div>
            </div>
          </>
        )}

        {actionType === "key" && (
          <div className="field">
            <label className="label">Key</label>
            <div className="input-with-button">
              <input
                type="text"
                value={keyName}
                onChange={(e) => tool().setKeyName(e.target.value)}
                disabled={!isIdle || capturingKey}
                className="input"
                placeholder="e.g., Space, Enter"
              />
              <button
                className="btn btn-pick"
                onClick={() => tool().captureKeyName()}
                disabled={!isIdle || capturingKey}
                title="Press the next key to capture it"
              >
                {capturingKey ? "Press..." : "Capture"}
              </button>
            </div>
          </div>
        )}

        {actionType === "key-to-click" && (
          <>
            <div className="field">
              <label className="label">Mouse Button</label>
              <select
                value={keyClickButton}
                onChange={(e) => tool().setKeyClickButton(e.target.value as MouseButton)}
                disabled={keyClickRunning}
                className="input"
              >
                <option value="Left">Left</option>
                <option value="Right">Right</option>
                <option value="Middle">Middle</option>
              </select>
            </div>
            <div className="field">
              <label className="label">Click Location</label>
              <select
                value={keyClickLocationMode}
                onChange={(e) => tool().setKeyClickLocationMode(e.target.value as KeyClickLocationMode)}
                disabled={keyClickRunning}
                className="input"
              >
                <option value="cursor">Cursor (follow mouse)</option>
                <option value="fixed">Fixed position</option>
              </select>
            </div>
            {keyClickLocationMode === "fixed" && (
              <>
                <div className="field">
                  <label className="label">X</label>
                  <input
                    type="number"
                    value={clickX}
                    onChange={(e) => tool().setClickX(parseInt(e.target.value, 10) || 0)}
                    disabled={keyClickRunning}
                    className="input"
                  />
                </div>
                <div className="field">
                  <label className="label">Y</label>
                  <div className="input-with-button">
                    <input
                      type="number"
                      value={clickY}
                      onChange={(e) => tool().setClickY(parseInt(e.target.value, 10) || 0)}
                      disabled={keyClickRunning}
                      className="input"
                    />
                    <button className="btn btn-pick" onClick={() => tool().openPicker()} disabled={keyClickRunning}>
                      Pick
                    </button>
                  </div>
                </div>
              </>
            )}
            <div className="field">
              <label className="label">Min Interval</label>
              <div className="input-suffix">
                <input
                  type="number"
                  value={keyClickInterval}
                  onChange={(e) => tool().setKeyClickInterval(parseInt(e.target.value, 10) || 40)}
                  disabled={keyClickRunning}
                  className="input"
                  min={10}
                  max={1000}
                />
                <span>ms</span>
              </div>
              <span className="field-hint">Rate limit between repeated clicks</span>
            </div>
            <div className="field">
              <label className="label">Hold Delay</label>
              <div className="input-suffix">
                <input
                  type="number"
                  value={keyClickHoldDelay}
                  onChange={(e) => tool().setKeyClickHoldDelay(parseInt(e.target.value, 10) || 150)}
                  disabled={keyClickRunning}
                  className="input"
                  min={0}
                />
                <span>ms</span>
              </div>
              <span className="field-hint">Time before repeat starts</span>
            </div>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={keyClickOnlyTargetFocused}
                onChange={(e) => tool().setKeyClickOnlyTargetFocused(e.target.checked)}
                disabled={keyClickRunning}
              />
              <span>Only click in the active window at start</span>
            </label>
            <div className="info-box">
              <strong>How it works</strong>
              <ul>
                <li>Tap A-Z: one click</li>
                <li>Hold A-Z: repeat after {keyClickHoldDelay}ms</li>
                <li>
                  Press <kbd>Space</kbd> to stop
                </li>
              </ul>
            </div>
            {keyClickRunning && (
              <div className="key-click-status">
                <span className="key-click-count">{keyClickCount} clicks</span>
                <span className="key-click-hint">Press Space to stop</span>
              </div>
            )}
          </>
        )}

        {actionType !== "key-to-click" && (
          <>
            <div className="field">
              <label className="label">Interval</label>
              <div className="input-suffix">
                <input
                  type="number"
                  value={intervalMs}
                  onChange={(e) => tool().setIntervalMs(parseInt(e.target.value, 10) || 100)}
                  disabled={!isIdle}
                  className="input"
                  min={50}
                />
                <span>ms</span>
              </div>
            </div>
            <div className="field">
              <label className="label">Repeat</label>
              <input
                type="text"
                value={repeatText}
                onChange={(e) => tool().setRepeatText(e.target.value)}
                disabled={!isIdle}
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
                  onChange={(e) => tool().setCountdownSecs(parseInt(e.target.value, 10) || 0)}
                  disabled={!isIdle}
                  className="input"
                  min={0}
                />
                <span>sec</span>
              </div>
            </div>
          </>
        )}
      </div>
    </>
  );
}
