import { useContext, useState } from "react";

import { ACTION_LABELS, actionKind, defaultActionForKind, NESTED_ACTION_GROUPS } from "../../lib/actions";
import { isValidIdentifier, uniqueName } from "../../lib/document";
import type { DslAction, DslActionKind, DslValue } from "../../lib/types";
import { useDocumentStore } from "../../stores/documentStore";
import { ConditionEditor } from "./ConditionEditor";
import { ButtonRow, NumberRow, OptionalNumberRow, ReferencesContext, TextRow, ValueInput, ValueRow } from "./fields";

interface KindGroup {
  label: string;
  kinds: DslActionKind[];
}

/**
 * The "Type" select. Changing the kind starts from that kind's defaults, so the parameters of the
 * old kind are dropped (the same as retyping the step).
 */
export function KindSelect(props: {
  action: DslAction;
  groups: KindGroup[];
  label: string;
  disabled: boolean;
  onChange: (action: DslAction) => void;
}) {
  const kind = actionKind(props.action);
  // An action the document already holds may not be offered here (a `call_macro` inside an `if`
  // from a hand-written file): keep showing what it is rather than pretending it is another kind.
  const listed = props.groups.some((g) => g.kinds.includes(kind));
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <select
        value={kind}
        onChange={(e) => props.onChange(defaultActionForKind(e.target.value as DslActionKind))}
        disabled={props.disabled}
        className="input"
      >
        {!listed && <option value={kind}>{ACTION_LABELS[kind]} (not allowed here)</option>}
        {props.groups.map((group) => (
          <optgroup key={group.label} label={group.label}>
            {group.kinds.map((k) => (
              <option key={k} value={k}>
                {ACTION_LABELS[k]}
              </option>
            ))}
          </optgroup>
        ))}
      </select>
    </div>
  );
}

interface ParamsProps {
  action: DslAction;
  disabled: boolean;
  onChange: (action: DslAction) => void;
}

/** The parameters of any action, edited in place. Used recursively for the branches of an `if`. */
export function ActionParams({ action, disabled, onChange }: ParamsProps) {
  const refs = useContext(ReferencesContext);

  if (typeof action === "string") {
    // `exit`: nothing to set.
    return <div className="field-note">Stops the macro at this point.</div>;
  }

  const kind = actionKind(action);

  /** Merge `patchObj` into the action's parameters. */
  function patch(patchObj: Record<string, unknown>): void {
    const inner = (action as Record<string, Record<string, unknown>>)[kind];
    onChange({ [kind]: { ...inner, ...patchObj } } as DslAction);
  }

  if ("click" in action || "double_click" in action || "mouse_down" in action || "mouse_up" in action) {
    const inner =
      "click" in action
        ? action.click
        : "double_click" in action
          ? action.double_click
          : "mouse_down" in action
            ? action.mouse_down
            : action.mouse_up;
    return (
      <>
        <ValueRow label="X" mode="number" value={inner.x} disabled={disabled} onChange={(x) => patch({ x })} />
        <ValueRow label="Y" mode="number" value={inner.y} disabled={disabled} onChange={(y) => patch({ y })} />
        <ButtonRow value={inner.button} disabled={disabled} onChange={(button) => patch({ button })} />
      </>
    );
  }

  if ("mouse_move" in action) {
    return (
      <>
        <ValueRow label="X" mode="number" value={action.mouse_move.x} disabled={disabled} onChange={(x) => patch({ x })} />
        <ValueRow label="Y" mode="number" value={action.mouse_move.y} disabled={disabled} onChange={(y) => patch({ y })} />
      </>
    );
  }

  if ("drag" in action) {
    const d = action.drag;
    return (
      <>
        <ValueRow label="From X" mode="number" value={d.from_x} disabled={disabled} onChange={(from_x) => patch({ from_x })} />
        <ValueRow label="From Y" mode="number" value={d.from_y} disabled={disabled} onChange={(from_y) => patch({ from_y })} />
        <ValueRow label="To X" mode="number" value={d.to_x} disabled={disabled} onChange={(to_x) => patch({ to_x })} />
        <ValueRow label="To Y" mode="number" value={d.to_y} disabled={disabled} onChange={(to_y) => patch({ to_y })} />
        <NumberRow
          label="Duration"
          value={d.duration_ms}
          disabled={disabled}
          min={0}
          onChange={(duration_ms) => patch({ duration_ms })}
        />
      </>
    );
  }

  if ("scroll" in action) {
    return (
      <>
        <ValueRow
          label="Delta X"
          mode="number"
          value={action.scroll.delta_x}
          disabled={disabled}
          onChange={(delta_x) => patch({ delta_x })}
        />
        <ValueRow
          label="Delta Y"
          mode="number"
          value={action.scroll.delta_y}
          disabled={disabled}
          onChange={(delta_y) => patch({ delta_y })}
        />
      </>
    );
  }

  if ("key_tap" in action || "key_down" in action || "key_up" in action) {
    const key = "key_tap" in action ? action.key_tap.key : "key_down" in action ? action.key_down.key : action.key_up.key;
    return <TextRow label="Key" value={key} disabled={disabled} onChange={(v) => patch({ key: v })} />;
  }

  if ("text_input" in action) {
    return (
      <ValueRow
        label="Text"
        mode="text"
        value={action.text_input.text}
        disabled={disabled}
        onChange={(text) => patch({ text })}
      />
    );
  }

  if ("wait" in action) {
    return <NumberRow label="Wait (ms)" value={action.wait.ms} disabled={disabled} min={0} onChange={(ms) => patch({ ms })} />;
  }

  if ("wait_until" in action) {
    const w = action.wait_until;
    return (
      <>
        <ConditionEditor condition={w.condition} disabled={disabled} onChange={(condition) => patch({ condition })} />
        <OptionalNumberRow
          label="Timeout (ms)"
          value={w.timeout_ms}
          placeholder="no limit"
          disabled={disabled}
          onChange={(timeout_ms) => patch({ timeout_ms })}
        />
        <NumberRow
          label="Poll (ms)"
          value={w.poll_interval_ms}
          disabled={disabled}
          min={1}
          onChange={(poll_interval_ms) => patch({ poll_interval_ms })}
        />
      </>
    );
  }

  if ("conditional" in action) {
    const c = action.conditional;
    return (
      <>
        <ConditionEditor condition={c.condition} disabled={disabled} onChange={(condition) => patch({ condition })} />
        <NestedAction label="Then" action={c.then_action} disabled={disabled} onChange={(then_action) => patch({ then_action })} />
        {c.else_action ? (
          <NestedAction
            label="Else"
            action={c.else_action}
            disabled={disabled}
            onChange={(else_action) => patch({ else_action })}
            onRemove={() => patch({ else_action: undefined })}
          />
        ) : (
          <button
            className="btn btn-sm nested-add"
            disabled={disabled}
            onClick={() => patch({ else_action: defaultActionForKind("wait") })}
          >
            + Add else
          </button>
        )}
      </>
    );
  }

  if ("set_counter" in action) {
    return (
      <>
        <TextRow
          label="Counter"
          value={action.set_counter.key}
          suggestions={refs.counters}
          disabled={disabled}
          onChange={(key) => patch({ key })}
        />
        <ValueRow
          label="Value"
          mode="number"
          value={action.set_counter.value}
          disabled={disabled}
          onChange={(value) => patch({ value })}
        />
      </>
    );
  }

  if ("incr_counter" in action || "decr_counter" in action || "reset_counter" in action) {
    const key = "incr_counter" in action ? action.incr_counter.key : "decr_counter" in action ? action.decr_counter.key : action.reset_counter.key;
    return <TextRow label="Counter" value={key} suggestions={refs.counters} disabled={disabled} onChange={(v) => patch({ key: v })} />;
  }

  if ("call_macro" in action) {
    return <CallMacroFields call={action.call_macro} disabled={disabled} onChange={onChange} />;
  }

  return null;
}

/** A branch of an `if`: its own Type select and parameters, one level deeper. */
function NestedAction(props: ParamsProps & { label: string; onRemove?: () => void }) {
  return (
    <div className="nested-action">
      <div className="nested-head">
        <span>{props.label}</span>
        {props.onRemove && (
          <button className="btn btn-sm" onClick={props.onRemove} disabled={props.disabled} title="Remove this branch">
            ×
          </button>
        )}
      </div>
      <KindSelect action={props.action} groups={NESTED_ACTION_GROUPS} label="Do" disabled={props.disabled} onChange={props.onChange} />
      <ActionParams action={props.action} disabled={props.disabled} onChange={props.onChange} />
    </div>
  );
}

/** `call_macro` with its arguments left out when there are none (the backend omits them too). */
function callMacro(name: string, args: Record<string, DslValue>): DslAction {
  return { call_macro: Object.keys(args).length > 0 ? { name, args } : { name } };
}

function CallMacroFields(props: {
  call: { name: string; args?: Record<string, DslValue> };
  disabled: boolean;
  onChange: (action: DslAction) => void;
}) {
  const profiles = useDocumentStore((s) => s.profiles);
  const ownName = useDocumentStore((s) => s.doc.name);
  const { name } = props.call;
  const args = props.call.args ?? {};
  const known = name === "" || profiles.includes(name);

  return (
    <>
      <div className="field">
        <label className="label">Macro</label>
        <select
          value={name}
          onChange={(e) => props.onChange(callMacro(e.target.value, args))}
          disabled={props.disabled}
          className="input"
        >
          <option value="">Choose a macro…</option>
          {!known && <option value={name}>{name} (not saved)</option>}
          {profiles.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>
        {!known && (
          <div className="field-hint field-hint-warn">No saved macro has this name; it is looked up by name when the run gets here.</div>
        )}
        {name !== "" && name === ownName && (
          <div className="field-hint field-hint-warn">This macro would call itself; the engine stops that with an error at run time.</div>
        )}
      </div>
      <ArgsEditor args={args} disabled={props.disabled} onChange={(next) => props.onChange(callMacro(name, next))} />
    </>
  );
}

/** The arguments of a `call_macro`: they become variables of the called macro. */
function ArgsEditor(props: { args: Record<string, DslValue>; disabled: boolean; onChange: (args: Record<string, DslValue>) => void }) {
  const names = Object.keys(props.args);

  function rename(from: string, to: string): void {
    props.onChange(Object.fromEntries(Object.entries(props.args).map(([k, v]) => [k === from ? to : k, v])));
  }

  return (
    <div className="args-editor">
      <div className="nested-head">
        <span>Arguments</span>
        <button
          className="btn btn-sm"
          disabled={props.disabled}
          onClick={() => props.onChange({ ...props.args, [uniqueName("arg", names)]: "" })}
          title="Pass a value to the called macro"
        >
          + Add
        </button>
      </div>
      {names.length === 0 && <div className="field-note">None: the called macro uses its own defaults.</div>}
      {names.map((name) => (
        <ArgRow
          key={name}
          name={name}
          value={props.args[name]}
          others={names.filter((n) => n !== name)}
          disabled={props.disabled}
          onRename={(to) => rename(name, to)}
          onValue={(value) => props.onChange({ ...props.args, [name]: value })}
          onRemove={() => props.onChange(Object.fromEntries(Object.entries(props.args).filter(([k]) => k !== name)))}
        />
      ))}
    </div>
  );
}

function ArgRow(props: {
  name: string;
  value: DslValue;
  others: string[];
  disabled: boolean;
  onRename: (to: string) => void;
  onValue: (value: DslValue) => void;
  onRemove: () => void;
}) {
  // Renaming commits on blur, so the name can be typed through states that are not names yet.
  const [draft, setDraft] = useState(props.name);
  const next = draft.trim();
  const valid = isValidIdentifier(next) && !props.others.includes(next);

  function commit(): void {
    if (next !== props.name && valid) props.onRename(next);
    else setDraft(props.name);
  }

  return (
    <div className="arg-row">
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        disabled={props.disabled}
        className="input"
        aria-label="Argument name"
        aria-invalid={valid ? undefined : true}
        title="The name of the variable in the called macro"
        spellCheck={false}
      />
      <ValueInput value={props.value} mode="auto" disabled={props.disabled} onChange={props.onValue} label={`Value of ${props.name}`} />
      <button className="btn btn-sm" onClick={props.onRemove} disabled={props.disabled} title="Remove this argument">
        ×
      </button>
    </div>
  );
}
