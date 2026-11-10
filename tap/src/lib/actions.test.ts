import { describe, expect, it } from "vitest";

import {
  ACTION_GROUPS,
  ACTION_LABELS,
  actionGlyph,
  actionKind,
  defaultActionForKind,
  formatDslAction,
  formatResolvedAction,
} from "./actions";
import type { ActionInfo, DslActionKind } from "./types";

const ALL_KINDS = Object.keys(ACTION_LABELS) as DslActionKind[];

describe("document actions", () => {
  it("builds a default action whose kind matches, for every kind", () => {
    for (const kind of ALL_KINDS) {
      expect(actionKind(defaultActionForKind(kind)), kind).toBe(kind);
    }
  });

  it("summarises and decorates every kind without falling back to raw JSON", () => {
    for (const kind of ALL_KINDS) {
      const action = defaultActionForKind(kind);
      expect(formatDslAction(action), kind).not.toMatch(/[{}]|undefined|object/);
      expect(actionGlyph(action), kind).not.toBe("");
    }
  });

  it("offers every kind exactly once, in groups", () => {
    const offered = ACTION_GROUPS.flatMap((g) => g.kinds);
    expect([...offered].sort()).toEqual([...ALL_KINDS].sort());
    expect(new Set(offered).size).toBe(offered.length);
    for (const group of ACTION_GROUPS) expect(group.kinds.length, group.label).toBeGreaterThan(0);
  });

  it("tells unit variants (bare strings) from payload variants", () => {
    expect(actionKind("exit")).toBe("exit");
    expect(actionKind({ wait: { ms: 5 } })).toBe("wait");
  });

  it("summarises representative actions, showing expressions verbatim", () => {
    expect(formatDslAction({ click: { x: 1, y: 2, button: "left" } })).toBe("Click @ (1, 2)");
    expect(formatDslAction({ click: { x: "{{ i * 10 }}", y: 2, button: "left" } })).toBe(
      "Click @ ({{ i * 10 }}, 2)",
    );
    expect(formatDslAction({ drag: { from_x: 1, from_y: 2, to_x: 3, to_y: 4, duration_ms: 5 } })).toBe(
      "Drag (1,2) → (3,4)",
    );
    expect(formatDslAction({ key_tap: { key: "Space" } })).toBe('Key "Space"');
    expect(formatDslAction({ wait: { ms: 500 } })).toBe("Wait 500ms");
  });

  it("summarises control flow, counters and macro calls", () => {
    expect(
      formatDslAction({ wait_until: { condition: { counter: { key: "n", op: ">=", value: 3 } }, timeout_ms: 5000, poll_interval_ms: 100 } }),
    ).toBe("Wait until counter n >= 3 (up to 5000ms)");
    expect(
      formatDslAction({
        conditional: { condition: "always", then_action: "exit", else_action: { wait: { ms: 1 } } },
      }),
    ).toBe("If always: Exit, else Wait 1ms");
    expect(formatDslAction({ set_counter: { key: "i", value: 0 } })).toBe("Set counter i = 0");
    expect(formatDslAction({ incr_counter: { key: "i" } })).toBe("Counter i +1");
    expect(formatDslAction({ call_macro: { name: "login", args: { a: 1, b: 2 } } })).toBe('Call "login" (2 args)');
    expect(formatDslAction({ call_macro: { name: "login" } })).toBe('Call "login"');
  });
});

describe("resolved actions (engine events)", () => {
  it("summarises representative actions", () => {
    expect(formatResolvedAction({ Click: { x: 1, y: 2, button: "Left" } })).toBe("Click @ (1, 2)");
    expect(formatResolvedAction({ Drag: { from: { x: 1, y: 2 }, to: { x: 3, y: 4 }, duration_ms: 5 } })).toBe(
      "Drag (1,2) → (3,4)",
    );
    expect(formatResolvedAction({ KeyTap: { key: "Space" } })).toBe('Key "Space"');
    expect(formatResolvedAction({ Wait: { ms: 500 } })).toBe("Wait 500ms");
  });

  it("falls back to JSON for resolved actions the log does not model", () => {
    const waitUntil = { WaitUntil: { timeout_ms: null } } as unknown as ActionInfo;
    expect(formatResolvedAction(waitUntil)).toBe('{"WaitUntil":{"timeout_ms":null}}');
  });
});
