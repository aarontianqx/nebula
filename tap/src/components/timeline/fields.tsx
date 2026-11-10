import { createContext, useContext, useEffect, useId, useRef, useState } from "react";

import { capitalize, MOUSE_BUTTONS } from "../../lib/actions";
import type { References } from "../../lib/document";
import { checkValueText, isDynamicValue, parseValueText, valueToText, withReference } from "../../lib/expression";
import type { ValueMode } from "../../lib/expression";
import type { ColorResponse, DslMouseButton, DslValue } from "../../lib/types";
import { usePickStore } from "../../stores/pickStore";

const NO_REFERENCES: References = { variables: [], counters: [] };

/** The variables and counters a value field may insert; provided once by the Inspector. */
export const ReferencesContext = createContext<References>(NO_REFERENCES);

interface RowProps {
  label: string;
  disabled: boolean;
}

/** An integer field (`0` when cleared). */
export function NumberRow(props: RowProps & { value: number; onChange: (value: number) => void; min?: number; max?: number }) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <input
        type="number"
        value={props.value}
        onChange={(e) => props.onChange(parseInt(e.target.value, 10) || 0)}
        disabled={props.disabled}
        className="input"
        min={props.min}
        max={props.max}
      />
    </div>
  );
}

/** An integer field where empty means "not set" (the field is then left out of the document). */
export function OptionalNumberRow(
  props: RowProps & { value: number | null | undefined; placeholder: string; onChange: (value: number | undefined) => void },
) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <input
        type="number"
        value={props.value ?? ""}
        onChange={(e) => {
          const n = parseInt(e.target.value, 10);
          props.onChange(Number.isNaN(n) ? undefined : Math.max(0, n));
        }}
        disabled={props.disabled}
        className="input"
        min={0}
        placeholder={props.placeholder}
      />
    </div>
  );
}

/**
 * The newest value of `value`, readable from a callback that outlives the render it was made in
 * (a key capture waits seconds, during which the step may be edited: the result must apply to
 * the step as it is then, not as it was when the button was pressed).
 */
function useLatest<T>(value: T) {
  const ref = useRef(value);
  useEffect(() => {
    ref.current = value;
  });
  return ref;
}

/** A key name field with a "Capture" button: press the key instead of typing its name. */
export function KeyRow(props: RowProps & { value: string; onChange: (value: string) => void }) {
  const id = useId();
  const capturing = usePickStore((s) => s.captureTarget === id);
  const busy = usePickStore((s) => s.captureTarget !== null);
  const onChange = useLatest(props.onChange);

  // A capture must not outlive the field that asked for it (the step was deleted or retyped).
  useEffect(
    () => () => {
      if (usePickStore.getState().captureTarget === id) usePickStore.getState().cancelCapture();
    },
    [id],
  );

  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <div className="input-with-button">
        <input
          type="text"
          value={props.value}
          onChange={(e) => props.onChange(e.target.value)}
          disabled={props.disabled}
          className="input"
          spellCheck={false}
        />
        <button
          className={`btn btn-pick ${capturing ? "picking" : ""}`}
          onClick={() => void usePickStore.getState().captureKey(id, (key) => onChange.current(key))}
          disabled={props.disabled || busy}
          title="Press the key you want to use (waits up to 10 seconds)"
        >
          {capturing ? "Press a key…" : "Capture"}
        </button>
      </div>
    </div>
  );
}

/**
 * A "pick on screen" button for a pair of coordinates, optionally with the color under the click
 * (`withColor`; `color` is then missing if it could not be read). A coordinate computed by an
 * expression is not overwritten: the button is off, with the reason shown, until both are plain
 * numbers.
 */
export function PickPositionRow(props: {
  label: string;
  buttonLabel: string;
  x: DslValue;
  y: DslValue;
  disabled: boolean;
  withColor?: boolean;
  onPicked: (x: number, y: number, color?: ColorResponse) => void;
}) {
  const id = useId();
  const picking = usePickStore((s) => s.positionTarget === id);
  const blocked = isDynamicValue(props.x) || isDynamicValue(props.y);
  const onPicked = useLatest(props.onPicked);

  // A request must not outlive the field that made it (the step was deleted or retyped).
  useEffect(
    () => () => {
      if (usePickStore.getState().positionTarget === id) usePickStore.getState().cancelPosition();
    },
    [id],
  );

  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <div className="value-cell">
        <button
          className={`btn btn-pick ${picking ? "picking" : ""}`}
          onClick={() =>
            void usePickStore
              .getState()
              .requestPosition(id, (x, y, color) => onPicked.current(x, y, color), { color: props.withColor })
          }
          disabled={props.disabled || blocked}
          title={`Click anywhere on the screen to take its position${props.withColor ? " and color" : ""} (Esc cancels)`}
        >
          {picking ? "Click on the screen…" : props.buttonLabel}
        </button>
        {blocked && <div className="value-hint">X or Y is an expression; make both plain numbers to pick them.</div>}
      </div>
    </div>
  );
}

export function TextRow(
  props: RowProps & { value: string; onChange: (value: string) => void; placeholder?: string; suggestions?: string[] },
) {
  const listId = useId();
  const suggestions = props.suggestions ?? [];
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <input
        type="text"
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        disabled={props.disabled}
        className="input"
        placeholder={props.placeholder}
        list={suggestions.length > 0 ? listId : undefined}
      />
      {suggestions.length > 0 && (
        <datalist id={listId}>
          {suggestions.map((s) => (
            <option key={s} value={s} />
          ))}
        </datalist>
      )}
    </div>
  );
}

export function SelectRow<T extends string>(
  props: RowProps & { value: T; options: readonly { value: T; label: string }[]; onChange: (value: T) => void },
) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <select
        value={props.value}
        onChange={(e) => props.onChange(e.target.value as T)}
        disabled={props.disabled}
        className="input"
      >
        {props.options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

export function ButtonRow(props: { value: DslMouseButton; disabled: boolean; onChange: (value: DslMouseButton) => void }) {
  return (
    <SelectRow
      label="Button"
      value={props.value}
      options={MOUSE_BUTTONS.map((b) => ({ value: b, label: capitalize(b) }))}
      disabled={props.disabled}
      onChange={props.onChange}
    />
  );
}

/**
 * The input of a `DslValue`: a literal, or a `{{ expression }}` over variables and counters.
 *
 * What is typed is kept as a draft while it still means the same value, so "1.0" is not
 * collapsed to "1" under the caret; any other change (another step selected, a variable
 * inserted) shows the value itself. Up / Down nudge a numeric literal by 1 (Shift: 10).
 */
export function ValueInput(props: {
  value: DslValue;
  mode: ValueMode;
  disabled: boolean;
  onChange: (value: DslValue) => void;
  placeholder?: string;
  label?: string;
}) {
  const { value, mode, disabled } = props;
  const refs = useContext(ReferencesContext);
  const [draft, setDraft] = useState<string | null>(null);

  const shown = draft !== null && parseValueText(draft, mode) === value ? draft : valueToText(value);
  const hint = disabled ? null : checkValueText(shown, mode);
  const hasRefs = refs.variables.length + refs.counters.length > 0;

  function commit(text: string): void {
    setDraft(text);
    props.onChange(parseValueText(text, mode));
  }

  return (
    <div className="value-cell">
      <div className="value-input">
        <input
          type="text"
          value={shown}
          onChange={(e) => commit(e.target.value)}
          onBlur={() => setDraft(null)}
          onKeyDown={(e) => {
            if ((e.key !== "ArrowUp" && e.key !== "ArrowDown") || typeof value !== "number") return;
            e.preventDefault();
            const step = e.shiftKey ? 10 : 1;
            commit(String(value + (e.key === "ArrowUp" ? step : -step)));
          }}
          disabled={disabled}
          className="input"
          placeholder={props.placeholder}
          aria-label={props.label}
          aria-invalid={hint ? true : undefined}
          spellCheck={false}
        />
        <select
          className="input value-ref"
          value=""
          onChange={(e) => e.target.value && commit(withReference(shown, e.target.value, mode))}
          disabled={disabled || !hasRefs}
          title={hasRefs ? "Insert a variable or counter" : "No variables or counters yet"}
          aria-label="Insert variable or counter"
        >
          <option value="">ƒ</option>
          {refs.variables.length > 0 && (
            <optgroup label="Variables">
              {refs.variables.map((name) => (
                <option key={`v:${name}`} value={name}>
                  {name}
                </option>
              ))}
            </optgroup>
          )}
          {refs.counters.length > 0 && (
            <optgroup label="Counters">
              {refs.counters.map((name) => (
                <option key={`c:${name}`} value={name}>
                  {name}
                </option>
              ))}
            </optgroup>
          )}
        </select>
      </div>
      {hint && <div className="value-hint">{hint}</div>}
    </div>
  );
}

/** A labelled [`ValueInput`]. */
export function ValueRow(props: RowProps & { value: DslValue; mode: ValueMode; onChange: (value: DslValue) => void }) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <ValueInput value={props.value} mode={props.mode} disabled={props.disabled} onChange={props.onChange} label={props.label} />
    </div>
  );
}
