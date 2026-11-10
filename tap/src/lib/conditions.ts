// Pure helpers over `DslCondition`: what the condition editor offers, and how conditions read.

import type { DslCondition, DslConditionKind } from "./types";

/** Human-readable label of every condition kind (typed exhaustively: a new variant must be named). */
export const CONDITION_LABELS: Record<DslConditionKind, string> = {
  window_focused: "Window is focused",
  window_exists: "Window exists",
  pixel_color: "Pixel color",
  counter: "Counter",
  always: "Always",
  never: "Never",
  and: "All of (and)",
  or: "Any of (or)",
  not: "Not",
};

/** The conditions the editor can create and edit field by field. */
export const LEAF_CONDITION_KINDS = [
  "window_focused",
  "window_exists",
  "pixel_color",
  "counter",
  "always",
  "never",
] as const satisfies readonly DslConditionKind[];

export type LeafConditionKind = (typeof LEAF_CONDITION_KINDS)[number];

/** Counter comparison operators, as the backend validates them. */
export const COMPARE_OPS = ["==", "!=", ">", "<", ">=", "<="] as const;

/** The variant name of a condition (unit variants are bare strings, the rest `{ kind: {...} }`). */
export function conditionKind(condition: DslCondition): DslConditionKind {
  return typeof condition === "string" ? condition : (Object.keys(condition)[0] as DslConditionKind);
}

/**
 * Whether the editor can edit this condition field by field. `and` / `or` / `not` combine other
 * conditions; they are shown, kept intact, and edited in the Code (YAML) view.
 */
export function isLeafCondition(condition: DslCondition): boolean {
  return (LEAF_CONDITION_KINDS as readonly string[]).includes(conditionKind(condition));
}

/** A starting point for a leaf condition (used when adding or switching the kind). */
export function defaultConditionForKind(kind: LeafConditionKind): DslCondition {
  switch (kind) {
    case "window_focused":
      // Left empty on purpose: the backend reports "requires either title or process" until the
      // user names a window, which is the prompt to do so.
      return { window_focused: {} };
    case "window_exists":
      return { window_exists: {} };
    case "pixel_color":
      return { pixel_color: { x: 0, y: 0, color: "#000000", tolerance: 10 } };
    case "counter":
      return { counter: { key: "counter", op: ">=", value: 1 } };
    case "always":
      return "always";
    case "never":
      return "never";
  }
}

/** `#RRGGBB`, the only color format the backend accepts. */
export function isHexColor(text: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(text);
}

function windowTarget(w: { title?: string | null; process?: string | null }): string {
  const parts = [w.title && `"${w.title}"`, w.process && `[${w.process}]`].filter(Boolean);
  return parts.length > 0 ? parts.join(" ") : "(any)";
}

/** One-line summary of a condition. */
export function formatCondition(condition: DslCondition): string {
  if (typeof condition === "string") return condition;
  if ("window_focused" in condition) return `window focused ${windowTarget(condition.window_focused)}`;
  if ("window_exists" in condition) return `window exists ${windowTarget(condition.window_exists)}`;
  if ("pixel_color" in condition) {
    const c = condition.pixel_color;
    return `pixel (${c.x}, ${c.y}) is ${c.color}`;
  }
  if ("counter" in condition) {
    const c = condition.counter;
    return `counter ${c.key} ${c.op} ${c.value}`;
  }
  if ("and" in condition) return `(${condition.and.map(formatCondition).join(" and ")})`;
  if ("or" in condition) return `(${condition.or.map(formatCondition).join(" or ")})`;
  return `not ${formatCondition(condition.not.condition)}`;
}
