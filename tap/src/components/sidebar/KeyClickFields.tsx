import type { KeyClickLocationMode } from "../../lib/types";
import { useKeyClickStore } from "../../stores/keyClickStore";
import { useSimpleStore } from "../../stores/simpleStore";
import { Hint, MouseButtonSelect, NumberWithUnit, PositionFields } from "./SimpleFormFields";

/** Settings of the Key->Click tool, locked while it runs. */
export function KeyClickFields() {
  const kc = useKeyClickStore();
  const simple = useSimpleStore();

  return (
    <>
      <MouseButtonSelect value={kc.button} disabled={kc.running} onChange={kc.setButton} />
      <div className="field">
        <label className="label">Click Location</label>
        <select
          value={kc.locationMode}
          onChange={(e) => kc.setLocationMode(e.target.value as KeyClickLocationMode)}
          disabled={kc.running}
          className="input"
        >
          <option value="cursor">Cursor (follow mouse)</option>
          <option value="fixed">Fixed position</option>
        </select>
      </div>
      {kc.locationMode === "fixed" && (
        <PositionFields
          x={simple.clickX}
          y={simple.clickY}
          disabled={kc.running}
          onXChange={simple.setClickX}
          onYChange={simple.setClickY}
          onPick={simple.openPicker}
        />
      )}
      <NumberWithUnit
        label="Min Interval"
        unit="ms"
        value={kc.intervalMs}
        fallback={40}
        min={10}
        max={1000}
        disabled={kc.running}
        hint="Rate limit between repeated clicks"
        onChange={kc.setIntervalMs}
      />
      <NumberWithUnit
        label="Hold Delay"
        unit="ms"
        value={kc.holdDelayMs}
        fallback={150}
        min={0}
        disabled={kc.running}
        hint="Time before repeat starts"
        onChange={kc.setHoldDelayMs}
      />
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={kc.onlyTargetFocused}
          onChange={(e) => kc.setOnlyTargetFocused(e.target.checked)}
          disabled={kc.running}
        />
        <span>Only click in the active window at start</span>
      </label>
      <Hint>
        <strong>How it works</strong>
        <ul>
          <li>Tap A-Z: one click</li>
          <li>Hold A-Z: repeat after {kc.holdDelayMs}ms</li>
          <li>
            Press <kbd>Space</kbd> to stop
          </li>
        </ul>
      </Hint>
      {kc.running && (
        <div className="key-click-status">
          <span className="key-click-count">{kc.count} clicks</span>
          <span className="key-click-hint">Press Space to stop</span>
        </div>
      )}
    </>
  );
}
