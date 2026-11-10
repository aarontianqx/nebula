import type { ReactNode } from "react";

import type { MouseButton } from "../../lib/types";

const BUTTONS: MouseButton[] = ["Left", "Right", "Middle"];

export function MouseButtonSelect(props: {
  value: MouseButton;
  disabled: boolean;
  onChange: (button: MouseButton) => void;
}) {
  return (
    <div className="field">
      <label className="label">Mouse Button</label>
      <select
        value={props.value}
        onChange={(e) => props.onChange(e.target.value as MouseButton)}
        disabled={props.disabled}
        className="input"
      >
        {BUTTONS.map((button) => (
          <option key={button} value={button}>
            {button}
          </option>
        ))}
      </select>
    </div>
  );
}

/** X and Y inputs with a button that picks the position on screen. */
export function PositionFields(props: {
  x: number;
  y: number;
  disabled: boolean;
  onXChange: (x: number) => void;
  onYChange: (y: number) => void;
  onPick: () => void;
}) {
  return (
    <>
      <div className="field">
        <label className="label">X</label>
        <input
          type="number"
          value={props.x}
          onChange={(e) => props.onXChange(parseInt(e.target.value, 10) || 0)}
          disabled={props.disabled}
          className="input"
        />
      </div>
      <div className="field">
        <label className="label">Y</label>
        <div className="input-with-button">
          <input
            type="number"
            value={props.y}
            onChange={(e) => props.onYChange(parseInt(e.target.value, 10) || 0)}
            disabled={props.disabled}
            className="input"
          />
          <button className="btn btn-pick" onClick={props.onPick} disabled={props.disabled}>
            Pick
          </button>
        </div>
      </div>
    </>
  );
}

/** A number input followed by its unit. An empty or zero entry falls back to `fallback`. */
export function NumberWithUnit(props: {
  label: string;
  unit: string;
  value: number;
  fallback: number;
  min?: number;
  max?: number;
  disabled: boolean;
  hint?: string;
  onChange: (value: number) => void;
}) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <div className="input-suffix">
        <input
          type="number"
          value={props.value}
          onChange={(e) => props.onChange(parseInt(e.target.value, 10) || props.fallback)}
          disabled={props.disabled}
          className="input"
          min={props.min}
          max={props.max}
        />
        <span>{props.unit}</span>
      </div>
      {props.hint && <span className="field-hint">{props.hint}</span>}
    </div>
  );
}

export function Hint(props: { children: ReactNode }) {
  return <div className="info-box">{props.children}</div>;
}
