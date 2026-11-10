import { useContext } from "react";

import {
  COMPARE_OPS,
  CONDITION_LABELS,
  conditionKind,
  defaultConditionForKind,
  formatCondition,
  isHexColor,
  isLeafCondition,
  LEAF_CONDITION_KINDS,
} from "../../lib/conditions";
import type { LeafConditionKind } from "../../lib/conditions";
import { orUndefined } from "../../lib/document";
import type { DslCondition } from "../../lib/types";
import { NumberRow, PickPositionRow, ReferencesContext, SelectRow, TextRow } from "./fields";

interface Props {
  condition: DslCondition;
  disabled: boolean;
  onChange: (condition: DslCondition) => void;
}

function clampTolerance(value: number): number {
  return Math.min(255, Math.max(0, value));
}

/**
 * Edits one condition. The leaf conditions are edited field by field; `and` / `or` / `not`
 * combine other conditions, so they are shown and kept as they are and edited in the Code
 * (YAML) view.
 */
export function ConditionEditor({ condition, disabled, onChange }: Props) {
  const kind = conditionKind(condition);
  const leaf = isLeafCondition(condition);

  return (
    <>
      <div className="field">
        <label className="label">Condition</label>
        <select
          value={kind}
          onChange={(e) => onChange(defaultConditionForKind(e.target.value as LeafConditionKind))}
          // Switching resets the parameters, so a combined condition (which the editor cannot
          // recreate) is never offered a switch that would silently destroy it.
          disabled={disabled || !leaf}
          className="input"
        >
          {leaf ? (
            LEAF_CONDITION_KINDS.map((k) => (
              <option key={k} value={k}>
                {CONDITION_LABELS[k]}
              </option>
            ))
          ) : (
            <option value={kind}>{CONDITION_LABELS[kind]}</option>
          )}
        </select>
      </div>
      {leaf ? (
        <LeafFields condition={condition} disabled={disabled} onChange={onChange} />
      ) : (
        <div className="info-box inspector-code-hint">
          <div className="inspector-summary">{formatCondition(condition)}</div>
          Combined conditions (and / or / not) are edited in the <strong>Code (YAML)</strong> view.
        </div>
      )}
    </>
  );
}

function LeafFields({ condition, disabled, onChange }: Props) {
  const refs = useContext(ReferencesContext);
  if (typeof condition === "string") return null; // always / never

  /** Merge `patch` into the condition's parameters. */
  function patch(patchObj: Record<string, unknown>): void {
    const kind = conditionKind(condition);
    const inner = (condition as unknown as Record<string, Record<string, unknown>>)[kind];
    onChange({ [kind]: { ...inner, ...patchObj } } as DslCondition);
  }

  if ("window_focused" in condition || "window_exists" in condition) {
    const w = "window_focused" in condition ? condition.window_focused : condition.window_exists;
    return (
      <>
        <TextRow
          label="Title"
          value={w.title ?? ""}
          placeholder="part of the title"
          disabled={disabled}
          onChange={(v) => patch({ title: orUndefined(v) })}
        />
        <TextRow
          label="Process"
          value={w.process ?? ""}
          placeholder="e.g. notepad.exe"
          disabled={disabled}
          onChange={(v) => patch({ process: orUndefined(v) })}
        />
      </>
    );
  }

  if ("pixel_color" in condition) {
    const p = condition.pixel_color;
    return (
      <>
        <NumberRow label="X" value={p.x} disabled={disabled} onChange={(x) => patch({ x })} />
        <NumberRow label="Y" value={p.y} disabled={disabled} onChange={(y) => patch({ y })} />
        <PickPositionRow
          label=""
          buttonLabel="Pick position and color"
          x={p.x}
          y={p.y}
          withColor
          disabled={disabled}
          onPicked={(x, y, color) => patch(color ? { x, y, color: color.hex } : { x, y })}
        />
        <div className="field">
          <label className="label">Color</label>
          <div className="value-input">
            <input
              type="text"
              value={p.color}
              onChange={(e) => patch({ color: e.target.value })}
              disabled={disabled}
              className="input"
              placeholder="#RRGGBB"
              maxLength={7}
              aria-invalid={isHexColor(p.color) ? undefined : true}
              spellCheck={false}
            />
            {isHexColor(p.color) && <span className="color-swatch" style={{ backgroundColor: p.color }} aria-hidden />}
          </div>
          {!isHexColor(p.color) && <div className="field-hint field-hint-warn">Use #RRGGBB, e.g. #FF8800.</div>}
        </div>
        <NumberRow
          label="Tolerance"
          value={p.tolerance}
          disabled={disabled}
          min={0}
          max={255}
          onChange={(tolerance) => patch({ tolerance: clampTolerance(tolerance) })}
        />
      </>
    );
  }

  if ("counter" in condition) {
    const c = condition.counter;
    return (
      <>
        <TextRow
          label="Counter"
          value={c.key}
          suggestions={refs.counters}
          disabled={disabled}
          onChange={(key) => patch({ key })}
        />
        <SelectRow
          label="Is"
          value={c.op}
          options={COMPARE_OPS.map((op) => ({ value: op, label: op }))}
          disabled={disabled}
          onChange={(op) => patch({ op })}
        />
        <NumberRow label="Value" value={c.value} disabled={disabled} onChange={(value) => patch({ value })} />
      </>
    );
  }

  return null;
}
