// Small pure helpers over `MacroDocument`, shared by the store and the components.
//
// The document comes straight from serde, which omits defaults (`enabled: true`, empty tags and
// variables, ...). Read through these helpers instead of poking at optional fields.

import type {
  DslAction,
  DslCondition,
  DslRunConfig,
  DslTimedAction,
  MacroDocument,
  ValidationErrorResponse,
  VariableDefinition,
  VariableType,
} from "./types";

/** The document shown before the backend has delivered the real one. */
export function emptyDocument(): MacroDocument {
  return {
    name: "Untitled",
    version: "1.0",
    timeline: [],
    run: { repeat: 0, start_delay_ms: 3000, speed: 1 },
  };
}

/** A step is enabled unless it says otherwise. */
export function isEnabled(step: DslTimedAction): boolean {
  return step.enabled !== false;
}

/** Declared variables as `[name, definition]`, sorted by name (serde's map order is arbitrary). */
export function variableEntries(doc: MacroDocument): [string, VariableDefinition][] {
  return Object.entries(doc.variables ?? {}).sort(([a], [b]) => a.localeCompare(b));
}

export function countVariables(doc: MacroDocument): number {
  return Object.keys(doc.variables ?? {}).length;
}

/** The "Repeat" field text: empty means forever. */
export function repeatText(run: DslRunConfig): string {
  return run.repeat === 0 ? "" : String(run.repeat);
}

/** Parse the "Repeat" field: empty means forever (`0`), anything else is at least once. */
export function parseRepeat(text: string): number {
  const trimmed = text.trim();
  if (trimmed === "") return 0;
  return Math.max(1, parseInt(trimmed, 10) || 1);
}

// === Names a `{{ expression }}` can use ===

/** What an expression may refer to: declared variables and the counters the macro uses. */
export interface References {
  variables: string[];
  counters: string[];
}

function visitCondition(condition: DslCondition, visit: (c: DslCondition) => void): void {
  visit(condition);
  if (typeof condition === "string") return;
  if ("and" in condition) condition.and.forEach((child) => visitCondition(child, visit));
  if ("or" in condition) condition.or.forEach((child) => visitCondition(child, visit));
  if ("not" in condition) visitCondition(condition.not.condition, visit);
}

function visitAction(action: DslAction, visit: (a: DslAction) => void, visitCond: (c: DslCondition) => void): void {
  visit(action);
  if (typeof action === "string") return;
  if ("wait_until" in action) visitCondition(action.wait_until.condition, visitCond);
  if ("conditional" in action) {
    visitCondition(action.conditional.condition, visitCond);
    visitAction(action.conditional.then_action, visit, visitCond);
    if (action.conditional.else_action) visitAction(action.conditional.else_action, visit, visitCond);
  }
}

/** Every counter the timeline sets, steps or tests (including inside `if` branches), sorted. */
export function collectCounterKeys(timeline: DslTimedAction[]): string[] {
  const keys = new Set<string>();
  const onAction = (a: DslAction) => {
    if (typeof a === "string") return;
    if ("set_counter" in a) keys.add(a.set_counter.key);
    if ("incr_counter" in a) keys.add(a.incr_counter.key);
    if ("decr_counter" in a) keys.add(a.decr_counter.key);
    if ("reset_counter" in a) keys.add(a.reset_counter.key);
  };
  const onCondition = (c: DslCondition) => {
    if (typeof c !== "string" && "counter" in c) keys.add(c.counter.key);
  };
  for (const step of timeline) visitAction(step.action, onAction, onCondition);
  keys.delete("");
  return [...keys].sort();
}

export function availableReferences(doc: MacroDocument): References {
  return {
    variables: variableEntries(doc).map(([name]) => name),
    counters: collectCounterKeys(doc.timeline),
  };
}

// === Validation findings ===

/** The findings that belong to timeline step `index` (the backend reports `timeline[i]...` paths). */
export function issuesForStep(issues: ValidationErrorResponse[], index: number): ValidationErrorResponse[] {
  // The closing bracket keeps `timeline[1]` from matching `timeline[12]`.
  const prefix = `timeline[${index}]`;
  return issues.filter((issue) => issue.path.startsWith(prefix));
}

// === Variable definitions ===

/** Identifier rule of the backend: a letter or `_`, then letters, digits or `_`. */
export function isValidIdentifier(name: string): boolean {
  return /^[\p{L}_][\p{L}\p{N}_]*$/u.test(name);
}

/** `base` if free, else `base_2`, `base_3`, ... */
export function uniqueName(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  if (!used.has(base)) return base;
  for (let n = 2; ; n += 1) {
    if (!used.has(`${base}_${n}`)) return `${base}_${n}`;
  }
}

/** The default of a new variable of this type (what the backend assumes when there is none). */
export function defaultForType(type: VariableType): string | number | boolean {
  return type === "number" ? 0 : type === "boolean" ? false : "";
}

/**
 * A default that matches `type`, converting the previous one where that is meaningful. The
 * backend silently falls back to `0` / `false` / `""` for a default of the wrong type, so the
 * editor must never store one.
 */
export function convertDefault(value: unknown, type: VariableType): string | number | boolean {
  if (type === "number") {
    const n = typeof value === "number" ? value : Number(value);
    return value === "" || value === null || value === undefined || !Number.isFinite(n) ? 0 : n;
  }
  if (type === "boolean") return value === true || value === "true";
  return value === null || value === undefined ? "" : String(value);
}

/** `record` with key `from` renamed to `to`, keeping its position. */
export function renameKey<T>(record: Record<string, T>, from: string, to: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).map(([key, value]) => [key === from ? to : key, value]));
}
