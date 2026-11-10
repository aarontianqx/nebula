import { describe, expect, it } from "vitest";

import {
  availableReferences,
  collectCounterKeys,
  convertDefault,
  countVariables,
  defaultForType,
  emptyDocument,
  isEnabled,
  isValidIdentifier,
  issuesForStep,
  parseRepeat,
  renameKey,
  repeatText,
  uniqueName,
  variableEntries,
} from "./document";
import type { DslTimedAction, ValidationErrorResponse } from "./types";

const step = (action: DslTimedAction["action"]): DslTimedAction => ({ at_ms: 0, action });

describe("document helpers", () => {
  it("treats a step as enabled unless it says otherwise (serde omits `enabled: true`)", () => {
    const action = { wait: { ms: 1 } } as const;
    expect(isEnabled({ at_ms: 0, action })).toBe(true);
    expect(isEnabled({ at_ms: 0, action, enabled: true })).toBe(true);
    expect(isEnabled({ at_ms: 0, action, enabled: false })).toBe(false);
  });

  it("lists variables sorted by name, tolerating a document without any", () => {
    const doc = emptyDocument();
    expect(variableEntries(doc)).toEqual([]);
    expect(countVariables(doc)).toBe(0);

    doc.variables = { b: { type: "number" }, a: { type: "string" } };
    expect(variableEntries(doc).map(([name]) => name)).toEqual(["a", "b"]);
    expect(countVariables(doc)).toBe(2);
  });

  it("maps the Repeat field to the run's `repeat` (0 means forever)", () => {
    expect(repeatText({ repeat: 0, start_delay_ms: 0, speed: 1 })).toBe("");
    expect(repeatText({ repeat: 4, start_delay_ms: 0, speed: 1 })).toBe("4");
    expect(parseRepeat("")).toBe(0);
    expect(parseRepeat("  ")).toBe(0);
    expect(parseRepeat("7")).toBe(7);
    expect(parseRepeat("0")).toBe(1);
    expect(parseRepeat("abc")).toBe(1);
  });
});

describe("names an expression can use", () => {
  it("collects the counters a timeline sets, steps and tests, nested ones included", () => {
    const timeline = [
      step({ set_counter: { key: "i", value: 0 } }),
      step({ incr_counter: { key: "hits" } }),
      step({ decr_counter: { key: "lives" } }),
      step({ reset_counter: { key: "i" } }),
      step({
        conditional: {
          condition: { and: [{ counter: { key: "seen", op: ">", value: 1 } }, { not: { condition: { counter: { key: "deep", op: "==", value: 0 } } } }] },
          then_action: { incr_counter: { key: "in_then" } },
          else_action: { conditional: { condition: "always", then_action: { set_counter: { key: "in_else", value: 1 } } } },
        },
      }),
      step({ wait_until: { condition: { or: [{ counter: { key: "waited", op: "<", value: 3 } }] }, poll_interval_ms: 100 } }),
      step({ wait: { ms: 1 } }),
      step("exit"),
      step({ incr_counter: { key: "" } }),
    ];
    expect(collectCounterKeys(timeline)).toEqual(["deep", "hits", "i", "in_else", "in_then", "lives", "seen", "waited"]);
  });

  it("offers declared variables (sorted) and counters separately", () => {
    const doc = emptyDocument();
    doc.variables = { z: { type: "number" }, a: { type: "string" } };
    doc.timeline = [step({ incr_counter: { key: "n" } })];
    expect(availableReferences(doc)).toEqual({ variables: ["a", "z"], counters: ["n"] });
    expect(availableReferences(emptyDocument())).toEqual({ variables: [], counters: [] });
  });
});

describe("findings of a step", () => {
  const issue = (path: string): ValidationErrorResponse => ({ path, message: path, line: null });

  it("keeps the findings under that step only, without confusing timeline[1] with timeline[12]", () => {
    const issues = [
      issue("name"),
      issue("timeline[1].action.x"),
      issue("timeline[1]"),
      issue("timeline[12].action.x"),
      issue("timeline[2].action.condition"),
    ];
    expect(issuesForStep(issues, 1).map((i) => i.path)).toEqual(["timeline[1].action.x", "timeline[1]"]);
    expect(issuesForStep(issues, 12).map((i) => i.path)).toEqual(["timeline[12].action.x"]);
    expect(issuesForStep(issues, 3)).toEqual([]);
  });
});

describe("variable definitions", () => {
  it("accepts the backend's identifiers and nothing else", () => {
    for (const ok of ["x", "_x", "user_name", "x1", "Größe"]) expect(isValidIdentifier(ok), ok).toBe(true);
    for (const bad of ["", "1x", "a-b", "a b", "a.b", "{{x}}"]) expect(isValidIdentifier(bad), bad).toBe(false);
  });

  it("picks a free name", () => {
    expect(uniqueName("variable", [])).toBe("variable");
    expect(uniqueName("variable", ["variable"])).toBe("variable_2");
    expect(uniqueName("variable", ["variable", "variable_2", "other"])).toBe("variable_3");
  });

  it("gives every type the default the backend would assume", () => {
    expect(defaultForType("string")).toBe("");
    expect(defaultForType("number")).toBe(0);
    expect(defaultForType("boolean")).toBe(false);
  });

  it("converts a default when the type changes, never leaving one of the wrong type", () => {
    expect(convertDefault("42", "number")).toBe(42);
    expect(convertDefault("1.5", "number")).toBe(1.5);
    expect(convertDefault("abc", "number")).toBe(0);
    expect(convertDefault("", "number")).toBe(0);
    expect(convertDefault(undefined, "number")).toBe(0);
    expect(convertDefault(true, "number")).toBe(1);
    expect(convertDefault(true, "string")).toBe("true");
    expect(convertDefault(7, "string")).toBe("7");
    expect(convertDefault(undefined, "string")).toBe("");
    expect(convertDefault("true", "boolean")).toBe(true);
    expect(convertDefault("yes", "boolean")).toBe(false);
    expect(convertDefault(1, "boolean")).toBe(false);
    expect(convertDefault(undefined, "boolean")).toBe(false);
  });

  it("renames a key in place", () => {
    const renamed = renameKey({ a: 1, b: 2, c: 3 }, "b", "x");
    expect(Object.keys(renamed)).toEqual(["a", "x", "c"]);
    expect(renamed).toEqual({ a: 1, x: 2, c: 3 });
  });
});
