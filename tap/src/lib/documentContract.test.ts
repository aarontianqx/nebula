// Contract between the hand-written TypeScript document types and serde's JSON.
//
// `crates/tap-core/tests/document_json_contract.rs` serialises a document that uses every action
// and condition variant. Here the same file is checked against `types.ts`:
//
// - the field tables below are typed with `keyof`, so a field that does not exist in the TS types
//   fails `tsc`; a field the Rust model adds or drops fails the equality checks;
// - the kind tables are `Record`s over the TS unions, so a kind missing on either side fails.

import { describe, expect, it } from "vitest";

import { ACTION_LABELS, actionGlyph, actionKind, defaultActionForKind, formatDslAction } from "./actions";
import fixture from "./__fixtures__/kitchen_sink.document.json";
import type {
  DslAction,
  DslActionKind,
  DslCondition,
  DslConditionKind,
  DslRunConfig,
  DslTargetWindow,
  DslTimedAction,
  MacroDocument,
  VariableDefinition,
} from "./types";

type PayloadOf<U, K extends string> = U extends Record<K, infer P> ? P : never;
type Fields<T> = readonly (keyof T & string)[];

const DOCUMENT_FIELDS: Fields<MacroDocument> = [
  "author",
  "description",
  "name",
  "run",
  "tags",
  "target_window",
  "timeline",
  "variables",
  "version",
];
const STEP_FIELDS: Fields<DslTimedAction> = ["action", "at_ms", "enabled", "note"];
const RUN_FIELDS: Fields<DslRunConfig> = ["repeat", "speed", "start_delay_ms"];
const TARGET_FIELDS: Fields<NonNullable<DslTargetWindow>> = ["coordinates", "pause_when_unfocused", "process", "title"];
const VARIABLE_FIELDS: Fields<VariableDefinition> = ["default", "description", "type"];

const ACTION_FIELDS: { [K in DslActionKind]: readonly (keyof PayloadOf<DslAction, K> & string)[] } = {
  click: ["button", "x", "y"],
  double_click: ["button", "x", "y"],
  mouse_down: ["button", "x", "y"],
  mouse_up: ["button", "x", "y"],
  mouse_move: ["x", "y"],
  drag: ["duration_ms", "from_x", "from_y", "to_x", "to_y"],
  scroll: ["delta_x", "delta_y"],
  key_tap: ["key"],
  key_down: ["key"],
  key_up: ["key"],
  text_input: ["text"],
  wait: ["ms"],
  wait_until: ["condition", "poll_interval_ms", "timeout_ms"],
  conditional: ["condition", "else_action", "then_action"],
  set_counter: ["key", "value"],
  incr_counter: ["key"],
  decr_counter: ["key"],
  reset_counter: ["key"],
  exit: [],
  call_macro: ["args", "name"],
};

const CONDITION_FIELDS: { [K in DslConditionKind]: readonly (keyof PayloadOf<DslCondition, K> & string)[] } = {
  window_focused: ["process", "title"],
  window_exists: ["process", "title"],
  pixel_color: ["color", "tolerance", "x", "y"],
  counter: ["key", "op", "value"],
  always: [],
  never: [],
  and: [],
  or: [],
  not: ["condition"],
};

// JSON imports widen string literals, so state explicitly that this is the wire format.
const doc = fixture as unknown as MacroDocument;

/** Payload keys seen per kind, across every occurrence (optional fields appear in some only). */
interface Seen {
  actions: Map<string, Set<string>>;
  conditions: Map<string, Set<string>>;
}

function note(into: Map<string, Set<string>>, kind: string, payload: unknown): void {
  const keys = into.get(kind) ?? new Set<string>();
  if (payload && typeof payload === "object" && !Array.isArray(payload)) {
    for (const key of Object.keys(payload)) keys.add(key);
  }
  into.set(kind, keys);
}

function walkCondition(c: DslCondition, seen: Seen): void {
  if (typeof c === "string") return note(seen.conditions, c, undefined);
  const kind = Object.keys(c)[0];
  const payload = (c as unknown as Record<string, unknown>)[kind];
  note(seen.conditions, kind, payload);
  if ("and" in c) c.and.forEach((child) => walkCondition(child, seen));
  if ("or" in c) c.or.forEach((child) => walkCondition(child, seen));
  if ("not" in c) walkCondition(c.not.condition, seen);
}

function walkAction(a: DslAction, seen: Seen): void {
  const kind = actionKind(a);
  note(seen.actions, kind, typeof a === "string" ? undefined : (a as unknown as Record<string, unknown>)[kind]);
  if (typeof a === "string") return;
  if ("wait_until" in a) walkCondition(a.wait_until.condition, seen);
  if ("conditional" in a) {
    walkCondition(a.conditional.condition, seen);
    walkAction(a.conditional.then_action, seen);
    if (a.conditional.else_action) walkAction(a.conditional.else_action, seen);
  }
}

function seenInFixture(): Seen {
  const seen: Seen = { actions: new Map(), conditions: new Map() };
  for (const step of doc.timeline) walkAction(step.action, seen);
  return seen;
}

function sorted(values: Iterable<string>): string[] {
  return [...values].sort();
}

describe("kitchen sink fixture (written by the Rust model)", () => {
  it("has exactly the document-level fields the TypeScript type declares", () => {
    expect(sorted(Object.keys(doc))).toEqual(sorted(DOCUMENT_FIELDS));
    expect(sorted(Object.keys(doc.run))).toEqual(sorted(RUN_FIELDS));
    expect(sorted(Object.keys(doc.target_window ?? {}))).toEqual(sorted(TARGET_FIELDS));
    const variable = Object.values(doc.variables ?? {})[0];
    expect(sorted(Object.keys(variable))).toEqual(sorted(VARIABLE_FIELDS));
  });

  it("has exactly the step fields the TypeScript type declares", () => {
    const keys = new Set(doc.timeline.flatMap((step) => Object.keys(step)));
    expect(sorted(keys)).toEqual(sorted(STEP_FIELDS));
  });

  it("covers every action kind, with exactly the fields the TypeScript type declares", () => {
    const { actions } = seenInFixture();
    expect(sorted(actions.keys())).toEqual(sorted(Object.keys(ACTION_FIELDS)));
    for (const [kind, keys] of actions) {
      expect(sorted(keys), `action ${kind}`).toEqual(sorted(ACTION_FIELDS[kind as DslActionKind]));
    }
  });

  it("covers every condition kind, with exactly the fields the TypeScript type declares", () => {
    const { conditions } = seenInFixture();
    expect(sorted(conditions.keys())).toEqual(sorted(Object.keys(CONDITION_FIELDS)));
    for (const [kind, keys] of conditions) {
      expect(sorted(keys), `condition ${kind}`).toEqual(sorted(CONDITION_FIELDS[kind as DslConditionKind]));
    }
  });

  it("is understood by every action helper", () => {
    for (const step of doc.timeline) {
      expect(formatDslAction(step.action)).not.toMatch(/undefined|\[object/);
      expect(actionGlyph(step.action)).not.toBe("");
      expect(ACTION_LABELS[actionKind(step.action)]).toBeTruthy();
    }
  });

  it("names every action kind the UI can create the way the backend does", () => {
    for (const kind of Object.keys(ACTION_LABELS) as DslActionKind[]) {
      expect(actionKind(defaultActionForKind(kind))).toBe(kind);
    }
  });
});
