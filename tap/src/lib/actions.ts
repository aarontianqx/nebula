import { formatCondition } from "./conditions";
import type { ActionInfo, DslAction, DslActionKind, DslMouseButton, DslValue } from "./types";

/** Makes a `switch` / `if` chain over a union exhaustive: a new variant fails `tsc` here. */
function assertNever(value: never): never {
  throw new Error(`Unhandled variant: ${JSON.stringify(value)}`);
}

interface ActionMeta {
  label: string;
  /** Short glyph the rail view uses to denote the action's category. */
  glyph: string;
  /** What the editor inserts when a step is added or its type is changed. */
  create: () => DslAction;
}

const MOUSE_GLYPH = "🖱";
const KEY_GLYPH = "⌨";
const COUNTER_GLYPH = "#";

/** Everything the editor knows about each action kind; typed exhaustively, so a new variant must be described. */
export const ACTION_META: Record<DslActionKind, ActionMeta> = {
  click: { label: "Click", glyph: MOUSE_GLYPH, create: () => ({ click: { x: 0, y: 0, button: "left" } }) },
  double_click: {
    label: "Double Click",
    glyph: MOUSE_GLYPH,
    create: () => ({ double_click: { x: 0, y: 0, button: "left" } }),
  },
  mouse_down: {
    label: "Mouse Down",
    glyph: MOUSE_GLYPH,
    create: () => ({ mouse_down: { x: 0, y: 0, button: "left" } }),
  },
  mouse_up: { label: "Mouse Up", glyph: MOUSE_GLYPH, create: () => ({ mouse_up: { x: 0, y: 0, button: "left" } }) },
  mouse_move: { label: "Move", glyph: MOUSE_GLYPH, create: () => ({ mouse_move: { x: 0, y: 0 } }) },
  drag: {
    label: "Drag",
    glyph: MOUSE_GLYPH,
    create: () => ({ drag: { from_x: 0, from_y: 0, to_x: 100, to_y: 100, duration_ms: 300 } }),
  },
  scroll: { label: "Scroll", glyph: "↕", create: () => ({ scroll: { delta_x: 0, delta_y: -3 } }) },
  key_tap: { label: "Key Tap", glyph: KEY_GLYPH, create: () => ({ key_tap: { key: "Space" } }) },
  key_down: { label: "Key Down", glyph: KEY_GLYPH, create: () => ({ key_down: { key: "Shift" } }) },
  key_up: { label: "Key Up", glyph: KEY_GLYPH, create: () => ({ key_up: { key: "Shift" } }) },
  text_input: { label: "Type Text", glyph: "✎", create: () => ({ text_input: { text: "" } }) },
  wait: { label: "Wait", glyph: "⏱", create: () => ({ wait: { ms: 500 } }) },
  wait_until: {
    label: "Wait Until",
    glyph: "⏳",
    create: () => ({ wait_until: { condition: "always", timeout_ms: 5000, poll_interval_ms: 100 } }),
  },
  conditional: {
    label: "If",
    glyph: "⑂",
    create: () => ({ conditional: { condition: "always", then_action: { wait: { ms: 0 } } } }),
  },
  set_counter: {
    label: "Set Counter",
    glyph: COUNTER_GLYPH,
    create: () => ({ set_counter: { key: "counter", value: 0 } }),
  },
  incr_counter: { label: "Counter +1", glyph: COUNTER_GLYPH, create: () => ({ incr_counter: { key: "counter" } }) },
  decr_counter: { label: "Counter -1", glyph: COUNTER_GLYPH, create: () => ({ decr_counter: { key: "counter" } }) },
  reset_counter: {
    label: "Reset Counter",
    glyph: COUNTER_GLYPH,
    create: () => ({ reset_counter: { key: "counter" } }),
  },
  exit: { label: "Exit", glyph: "⏹", create: () => "exit" },
  call_macro: { label: "Call Macro", glyph: "↪", create: () => ({ call_macro: { name: "" } }) },
};

/** The kinds the editor offers, grouped the way the "Add" menu and the Type select show them. */
export const ACTION_GROUPS: { label: string; kinds: DslActionKind[] }[] = [
  {
    label: "Input",
    kinds: [
      "click",
      "double_click",
      "mouse_down",
      "mouse_up",
      "mouse_move",
      "drag",
      "scroll",
      "key_tap",
      "key_down",
      "key_up",
      "text_input",
    ],
  },
  { label: "Timing", kinds: ["wait", "wait_until"] },
  { label: "Control flow", kinds: ["conditional", "exit"] },
  { label: "Counters", kinds: ["set_counter", "incr_counter", "decr_counter", "reset_counter"] },
  { label: "Macro", kinds: ["call_macro"] },
];

/** The variant name of an action (unit variants are bare strings, the rest `{ kind: {...} }`). */
export function actionKind(action: DslAction): DslActionKind {
  return typeof action === "string" ? action : (Object.keys(action)[0] as DslActionKind);
}

export function defaultActionForKind(kind: DslActionKind): DslAction {
  return ACTION_META[kind].create();
}

export const MOUSE_BUTTONS: DslMouseButton[] = ["left", "right", "middle"];

export function capitalize(word: string): string {
  return word.charAt(0).toUpperCase() + word.slice(1);
}

/** A value as it should read in a summary: literals as-is, expressions verbatim. */
export function formatValue(value: DslValue): string {
  return String(value);
}

/** Human-readable one-line summary of a document action. */
export function formatDslAction(action: DslAction): string {
  if (typeof action === "string") return ACTION_META[action].label;
  if ("click" in action) return `Click @ (${formatValue(action.click.x)}, ${formatValue(action.click.y)})`;
  if ("double_click" in action) {
    return `DblClick @ (${formatValue(action.double_click.x)}, ${formatValue(action.double_click.y)})`;
  }
  if ("mouse_down" in action) {
    return `MouseDown @ (${formatValue(action.mouse_down.x)}, ${formatValue(action.mouse_down.y)})`;
  }
  if ("mouse_up" in action) {
    return `MouseUp @ (${formatValue(action.mouse_up.x)}, ${formatValue(action.mouse_up.y)})`;
  }
  if ("mouse_move" in action) {
    return `Move → (${formatValue(action.mouse_move.x)}, ${formatValue(action.mouse_move.y)})`;
  }
  if ("drag" in action) {
    const d = action.drag;
    return `Drag (${formatValue(d.from_x)},${formatValue(d.from_y)}) → (${formatValue(d.to_x)},${formatValue(d.to_y)})`;
  }
  if ("scroll" in action) {
    return `Scroll (${formatValue(action.scroll.delta_x)}, ${formatValue(action.scroll.delta_y)})`;
  }
  if ("key_tap" in action) return `Key "${action.key_tap.key}"`;
  if ("key_down" in action) return `KeyDown "${action.key_down.key}"`;
  if ("key_up" in action) return `KeyUp "${action.key_up.key}"`;
  if ("text_input" in action) return `Type "${formatValue(action.text_input.text)}"`;
  if ("wait" in action) return `Wait ${action.wait.ms}ms`;
  if ("wait_until" in action) {
    const w = action.wait_until;
    const limit = w.timeout_ms != null ? ` (up to ${w.timeout_ms}ms)` : "";
    return `Wait until ${formatCondition(w.condition)}${limit}`;
  }
  if ("conditional" in action) {
    const c = action.conditional;
    const otherwise = c.else_action ? `, else ${formatDslAction(c.else_action)}` : "";
    return `If ${formatCondition(c.condition)}: ${formatDslAction(c.then_action)}${otherwise}`;
  }
  if ("set_counter" in action) {
    return `Set counter ${action.set_counter.key} = ${formatValue(action.set_counter.value)}`;
  }
  if ("incr_counter" in action) return `Counter ${action.incr_counter.key} +1`;
  if ("decr_counter" in action) return `Counter ${action.decr_counter.key} -1`;
  if ("reset_counter" in action) return `Reset counter ${action.reset_counter.key}`;
  if ("call_macro" in action) {
    const args = Object.keys(action.call_macro.args ?? {}).length;
    return `Call "${action.call_macro.name}"${args > 0 ? ` (${args} arg${args === 1 ? "" : "s"})` : ""}`;
  }
  return assertNever(action);
}

export function actionGlyph(action: DslAction): string {
  return ACTION_META[actionKind(action)].glyph;
}

/** One-line summary of a *resolved* action, as reported by the engine while it runs. */
export function formatResolvedAction(action: ActionInfo): string {
  if ("Click" in action) return `Click @ (${action.Click.x}, ${action.Click.y})`;
  if ("DoubleClick" in action) return `DblClick @ (${action.DoubleClick.x}, ${action.DoubleClick.y})`;
  if ("MouseDown" in action) return `MouseDown @ (${action.MouseDown.x}, ${action.MouseDown.y})`;
  if ("MouseUp" in action) return `MouseUp @ (${action.MouseUp.x}, ${action.MouseUp.y})`;
  if ("MouseMove" in action) return `Move → (${action.MouseMove.x}, ${action.MouseMove.y})`;
  if ("Drag" in action)
    return `Drag (${action.Drag.from.x},${action.Drag.from.y}) → (${action.Drag.to.x},${action.Drag.to.y})`;
  if ("KeyTap" in action) return `Key "${action.KeyTap.key}"`;
  if ("KeyDown" in action) return `KeyDown "${action.KeyDown.key}"`;
  if ("KeyUp" in action) return `KeyUp "${action.KeyUp.key}"`;
  if ("TextInput" in action) return `Type "${action.TextInput.text}"`;
  if ("Wait" in action) return `Wait ${action.Wait.ms}ms`;
  if ("Scroll" in action) return `Scroll (${action.Scroll.delta_x}, ${action.Scroll.delta_y})`;
  return JSON.stringify(action);
}

export function formatDuration(ms: number): string {
  const secs = Math.floor(ms / 1000);
  const mins = Math.floor(secs / 60);
  const remSecs = secs % 60;
  const remMs = ms % 1000;
  if (mins > 0) {
    return `${mins}:${remSecs.toString().padStart(2, "0")}.${Math.floor(remMs / 100)}`;
  }
  return `${secs}.${Math.floor(remMs / 100)}s`;
}
