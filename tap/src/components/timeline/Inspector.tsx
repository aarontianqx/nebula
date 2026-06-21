import { actionKind, defaultActionForKind, ACTION_KINDS, MOUSE_BUTTONS } from "../../lib/actions";
import type { ActionInfo, ActionKind, MouseButton } from "../../lib/types";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";

export function Inspector() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const timeline = useDocumentStore((s) => s.timeline);
  const editable = useDocumentStore((s) => s.editable);
  const doc = useDocumentStore.getState;
  const selectedIdx = useUiStore((s) => s.selectedActionIdx);

  const canEdit = isIdle && editable;
  const item = selectedIdx !== null ? timeline[selectedIdx] : undefined;

  if (selectedIdx === null || !item) {
    return (
      <div className="card inspector-card">
        <div className="inspector-empty">Select an action to edit its parameters.</div>
      </div>
    );
  }

  const idx = selectedIdx;
  const action = item.action;
  const kind = actionKind(action);

  function patch(patchObj: Record<string, unknown>): void {
    const inner = (action as Record<string, Record<string, unknown>>)[kind];
    doc().setAction(idx, { [kind]: { ...inner, ...patchObj } } as ActionInfo);
  }

  return (
    <div className="card inspector-card">
      <div className="inspector-head">
        <h4>Inspector</h4>
        <span className="inspector-index">#{idx + 1}</span>
      </div>

      <div className="field">
        <label className="label">Type</label>
        <select
          value={kind}
          onChange={(e) => doc().setAction(idx, defaultActionForKind(e.target.value as ActionKind))}
          disabled={!canEdit}
          className="input"
        >
          {ACTION_KINDS.map((k) => (
            <option key={k.kind} value={k.kind}>
              {k.label}
            </option>
          ))}
        </select>
      </div>

      <div className="field">
        <label className="label">At (ms)</label>
        <input
          type="number"
          value={item.at_ms}
          onChange={(e) => doc().setAtMs(idx, parseInt(e.target.value, 10) || 0)}
          disabled={!canEdit}
          className="input"
          min={0}
        />
      </div>

      {renderParams(action, canEdit, patch, (a) => doc().setAction(idx, a))}

      <div className="field">
        <label className="label">Note</label>
        <input
          type="text"
          value={item.note ?? ""}
          onChange={(e) => doc().setNote(idx, e.target.value)}
          disabled={!canEdit}
          className="input"
          placeholder="optional"
        />
      </div>
    </div>
  );
}

function renderParams(
  action: ActionInfo,
  disabled: boolean,
  patch: (patchObj: Record<string, unknown>) => void,
  setAction: (a: ActionInfo) => void
) {
  const dis = !disabled;
  if ("Click" in action || "DoubleClick" in action || "MouseDown" in action || "MouseUp" in action) {
    const inner = "Click" in action
      ? action.Click
      : "DoubleClick" in action
        ? action.DoubleClick
        : "MouseDown" in action
          ? action.MouseDown
          : action.MouseUp;
    return (
      <>
        <NumberRow label="X" value={inner.x} disabled={dis} onChange={(x) => patch({ x })} />
        <NumberRow label="Y" value={inner.y} disabled={dis} onChange={(y) => patch({ y })} />
        <ButtonRow value={inner.button} disabled={dis} onChange={(button) => patch({ button })} />
      </>
    );
  }
  if ("MouseMove" in action) {
    return (
      <>
        <NumberRow label="X" value={action.MouseMove.x} disabled={dis} onChange={(x) => patch({ x })} />
        <NumberRow label="Y" value={action.MouseMove.y} disabled={dis} onChange={(y) => patch({ y })} />
      </>
    );
  }
  if ("Drag" in action) {
    const d = action.Drag;
    return (
      <>
        <NumberRow label="From X" value={d.from.x} disabled={dis} onChange={(x) => setAction({ Drag: { ...d, from: { ...d.from, x } } })} />
        <NumberRow label="From Y" value={d.from.y} disabled={dis} onChange={(y) => setAction({ Drag: { ...d, from: { ...d.from, y } } })} />
        <NumberRow label="To X" value={d.to.x} disabled={dis} onChange={(x) => setAction({ Drag: { ...d, to: { ...d.to, x } } })} />
        <NumberRow label="To Y" value={d.to.y} disabled={dis} onChange={(y) => setAction({ Drag: { ...d, to: { ...d.to, y } } })} />
        <NumberRow label="Duration" value={d.duration_ms} disabled={dis} onChange={(duration_ms) => patch({ duration_ms })} />
      </>
    );
  }
  if ("KeyTap" in action || "KeyDown" in action || "KeyUp" in action) {
    const key = "KeyTap" in action ? action.KeyTap.key : "KeyDown" in action ? action.KeyDown.key : action.KeyUp.key;
    return <TextRow label="Key" value={key} disabled={dis} onChange={(v) => patch({ key: v })} />;
  }
  if ("TextInput" in action) {
    return <TextRow label="Text" value={action.TextInput.text} disabled={dis} onChange={(v) => patch({ text: v })} />;
  }
  if ("Wait" in action) {
    return <NumberRow label="Wait (ms)" value={action.Wait.ms} disabled={dis} onChange={(ms) => patch({ ms })} />;
  }
  if ("Scroll" in action) {
    return (
      <>
        <NumberRow label="Delta X" value={action.Scroll.delta_x} disabled={dis} onChange={(delta_x) => patch({ delta_x })} />
        <NumberRow label="Delta Y" value={action.Scroll.delta_y} disabled={dis} onChange={(delta_y) => patch({ delta_y })} />
      </>
    );
  }
  return null;
}

function NumberRow(props: { label: string; value: number; disabled: boolean; onChange: (value: number) => void }) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <input
        type="number"
        value={props.value}
        onChange={(e) => props.onChange(parseInt(e.target.value, 10) || 0)}
        disabled={props.disabled}
        className="input"
      />
    </div>
  );
}

function TextRow(props: { label: string; value: string; disabled: boolean; onChange: (value: string) => void }) {
  return (
    <div className="field">
      <label className="label">{props.label}</label>
      <input
        type="text"
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        disabled={props.disabled}
        className="input"
      />
    </div>
  );
}

function ButtonRow(props: { value: MouseButton; disabled: boolean; onChange: (value: MouseButton) => void }) {
  return (
    <div className="field">
      <label className="label">Button</label>
      <select
        value={props.value}
        onChange={(e) => props.onChange(e.target.value as MouseButton)}
        disabled={props.disabled}
        className="input"
      >
        {MOUSE_BUTTONS.map((b) => (
          <option key={b} value={b}>
            {b}
          </option>
        ))}
      </select>
    </div>
  );
}
