import type { ActionInfo, ActionKind, MouseButton } from "./types";

/** Ordered list of action kinds for the Inspector type selector. */
export const ACTION_KINDS: { kind: ActionKind; label: string }[] = [
  { kind: "Click", label: "Click" },
  { kind: "DoubleClick", label: "Double Click" },
  { kind: "MouseDown", label: "Mouse Down" },
  { kind: "MouseUp", label: "Mouse Up" },
  { kind: "MouseMove", label: "Move" },
  { kind: "Drag", label: "Drag" },
  { kind: "KeyTap", label: "Key Tap" },
  { kind: "KeyDown", label: "Key Down" },
  { kind: "KeyUp", label: "Key Up" },
  { kind: "TextInput", label: "Type Text" },
  { kind: "Wait", label: "Wait" },
  { kind: "Scroll", label: "Scroll" },
];

/** Return the discriminant key of an externally-tagged action. */
export function actionKind(action: ActionInfo): ActionKind {
  return Object.keys(action)[0] as ActionKind;
}

/** A sensible default action for a given kind (used when adding/changing type). */
export function defaultActionForKind(kind: ActionKind): ActionInfo {
  const button: MouseButton = "Left";
  switch (kind) {
    case "Click":
      return { Click: { x: 0, y: 0, button } };
    case "DoubleClick":
      return { DoubleClick: { x: 0, y: 0, button } };
    case "MouseDown":
      return { MouseDown: { x: 0, y: 0, button } };
    case "MouseUp":
      return { MouseUp: { x: 0, y: 0, button } };
    case "MouseMove":
      return { MouseMove: { x: 0, y: 0 } };
    case "Drag":
      return { Drag: { from: { x: 0, y: 0 }, to: { x: 100, y: 100 }, duration_ms: 300 } };
    case "KeyTap":
      return { KeyTap: { key: "Space" } };
    case "KeyDown":
      return { KeyDown: { key: "Shift" } };
    case "KeyUp":
      return { KeyUp: { key: "Shift" } };
    case "TextInput":
      return { TextInput: { text: "" } };
    case "Wait":
      return { Wait: { ms: 500 } };
    case "Scroll":
      return { Scroll: { delta_x: 0, delta_y: -3 } };
  }
}

export const MOUSE_BUTTONS: MouseButton[] = ["Left", "Right", "Middle"];

/** Human-readable one-line summary of an action. */
export function formatAction(action: ActionInfo): string {
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

/** Short glyph used by the rail view to denote action category. */
export function actionGlyph(action: ActionInfo): string {
  const kind = actionKind(action);
  if (kind.startsWith("Mouse") || kind === "Click" || kind === "DoubleClick" || kind === "Drag") return "🖱";
  if (kind.startsWith("Key")) return "⌨";
  if (kind === "TextInput") return "✎";
  if (kind === "Wait") return "⏱";
  if (kind === "Scroll") return "↕";
  return "•";
}

export function formatTime(): string {
  const now = new Date();
  const h = now.getHours().toString().padStart(2, "0");
  const m = now.getMinutes().toString().padStart(2, "0");
  const s = now.getSeconds().toString().padStart(2, "0");
  const ms = now.getMilliseconds().toString().padStart(3, "0");
  return `${h}:${m}:${s}.${ms}`;
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
