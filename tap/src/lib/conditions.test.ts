import { describe, expect, it } from "vitest";

import {
  COMPARE_OPS,
  CONDITION_LABELS,
  conditionKind,
  defaultConditionForKind,
  formatCondition,
  isHexColor,
  isLeafCondition,
  LEAF_CONDITION_KINDS,
} from "./conditions";
import type { DslCondition, DslConditionKind } from "./types";

const ALL_KINDS = Object.keys(CONDITION_LABELS) as DslConditionKind[];

describe("conditions", () => {
  it("tells unit variants (bare strings) from payload variants", () => {
    expect(conditionKind("always")).toBe("always");
    expect(conditionKind({ counter: { key: "n", op: "==", value: 1 } })).toBe("counter");
    expect(conditionKind({ not: { condition: "never" } })).toBe("not");
  });

  it("edits the leaf conditions field by field and leaves the combinators to the Code view", () => {
    for (const kind of LEAF_CONDITION_KINDS) {
      const condition = defaultConditionForKind(kind);
      expect(conditionKind(condition), kind).toBe(kind);
      expect(isLeafCondition(condition), kind).toBe(true);
    }
    expect(isLeafCondition({ and: ["always"] })).toBe(false);
    expect(isLeafCondition({ or: ["always"] })).toBe(false);
    expect(isLeafCondition({ not: { condition: "always" } })).toBe(false);
    // Every kind is either editable or one of the combinators: none can fall through.
    const combinators = ALL_KINDS.filter((k) => !(LEAF_CONDITION_KINDS as readonly string[]).includes(k));
    expect(combinators.sort()).toEqual(["and", "not", "or"]);
  });

  it("starts a window condition without a target, so the backend asks for one", () => {
    expect(defaultConditionForKind("window_focused")).toEqual({ window_focused: {} });
    expect(defaultConditionForKind("window_exists")).toEqual({ window_exists: {} });
  });

  it("starts the other leaves with values the backend accepts", () => {
    const pixel = defaultConditionForKind("pixel_color");
    expect(pixel).toEqual({ pixel_color: { x: 0, y: 0, color: "#000000", tolerance: 10 } });
    const counter = defaultConditionForKind("counter") as { counter: { op: string } };
    expect(COMPARE_OPS as readonly string[]).toContain(counter.counter.op);
  });

  it("recognises #RRGGBB and nothing else", () => {
    expect(isHexColor("#FF8800")).toBe(true);
    expect(isHexColor("#ff8800")).toBe(true);
    for (const bad of ["FF8800", "#FF880", "#FF88000", "#GG8800", "", "#12345g"]) {
      expect(isHexColor(bad), bad).toBe(false);
    }
  });

  it("summarises every kind without raw JSON", () => {
    const samples: DslCondition[] = [
      ...LEAF_CONDITION_KINDS.map((k) => defaultConditionForKind(k)),
      { and: ["always", "never"] },
      { or: ["always", "never"] },
      { not: { condition: "never" } },
    ];
    for (const condition of samples) {
      expect(formatCondition(condition), conditionKind(condition)).not.toMatch(/[{}]|undefined|object/);
    }
  });

  it("formats nested conditions", () => {
    expect(
      formatCondition({
        and: [{ not: { condition: "never" } }, { pixel_color: { x: 1, y: 2, color: "#FF0000", tolerance: 5 } }],
      }),
    ).toBe("(not never and pixel (1, 2) is #FF0000)");
    expect(formatCondition({ window_focused: { title: "Notepad", process: null } })).toBe('window focused "Notepad"');
    expect(formatCondition({ window_exists: {} })).toBe("window exists (any)");
    expect(formatCondition({ counter: { key: "n", op: ">=", value: 3 } })).toBe("counter n >= 3");
  });
});
