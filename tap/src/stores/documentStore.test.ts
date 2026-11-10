// Contract tests for the document sync protocol (see AGENTS.md "Sync contract"):
//
//   visual edit -> local mirror of the MacroDocument -> debounced `update_document` push
//   push is force-flushed before Start, Save, Export and Code view
//   a document replaced by the backend drops any pending push
//   the mirror IS the document: nothing is projected, so nothing can be lost

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import fixture from "../lib/__fixtures__/kitchen_sink.document.json";
import type { MacroDocument } from "../lib/types";
import { click, deferred, freshStores, macro, primeBackend, wait } from "../test/harness";

vi.mock("../lib/ipc", async () => (await import("../test/ipcMock")).createIpcMock());

const PUSH_DEBOUNCE_MS = 250;

type Stores = Awaited<ReturnType<typeof freshStores>>;

/** Fresh stores with a two-step macro already mirrored. */
async function setup(): Promise<Stores> {
  const stores = await freshStores();
  primeBackend(stores.api);
  stores.useDocumentStore.setState({ doc: macro("Mirror", [click(1, 0), click(2, 100)]) });
  return stores;
}

function doc(stores: Stores): MacroDocument {
  return stores.useDocumentStore.getState().doc;
}

/** The document the `n`th push carried. */
function pushed(stores: Stores, n = 0): MacroDocument {
  return stores.api.updateDocument.mock.calls[n][0];
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("debounced push", () => {
  it("coalesces a burst of edits into one push carrying the latest state", async () => {
    const stores = await setup();
    const { api, useDocumentStore } = stores;
    const store = useDocumentStore.getState();

    store.setAtMs(0, 10);
    store.setAtMs(0, 20);
    store.setAtMs(0, 30);

    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS - 1);
    expect(api.updateDocument).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(api.updateDocument).toHaveBeenCalledTimes(1);
    expect(pushed(stores).timeline[0].at_ms).toBe(30);
  });

  it("restarts the debounce window on every edit", async () => {
    const { api, useDocumentStore } = await setup();
    const store = useDocumentStore.getState();

    store.setAtMs(0, 10);
    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS - 50);
    store.setAtMs(0, 20);
    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS - 50);
    expect(api.updateDocument).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(50);
    expect(api.updateDocument).toHaveBeenCalledTimes(1);
  });

  it("carries metadata and timeline edits in one push: they are the same document", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setDescription("hello");
    store.setAuthor("me");
    store.setTags(["a", "b"]);
    store.setAtMs(0, 10);
    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS);

    expect(stores.api.updateDocument).toHaveBeenCalledTimes(1);
    const sent = pushed(stores);
    expect([sent.description, sent.author, sent.tags, sent.timeline[0].at_ms]).toEqual(["hello", "me", ["a", "b"], 10]);
  });

  it("drops emptied optional fields instead of sending blanks", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setDescription("x");
    store.setDescription("");
    await store.flush();

    // What matters is the wire shape: an emptied field must not reach serde as a blank string.
    const wire = JSON.parse(JSON.stringify(pushed(stores))) as Record<string, unknown>;
    expect(wire).not.toHaveProperty("description");
  });
});

describe("flush", () => {
  it("pushes pending edits immediately and leaves no timer behind", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setAtMs(0, 42);
    await store.flush();

    expect(stores.api.updateDocument).toHaveBeenCalledTimes(1);
    expect(pushed(stores).timeline[0].at_ms).toBe(42);

    await vi.advanceTimersByTimeAsync(5_000);
    expect(stores.api.updateDocument).toHaveBeenCalledTimes(1);
  });

  it("is a no-op when nothing is pending", async () => {
    const { api, useDocumentStore } = await setup();
    await useDocumentStore.getState().flush();
    expect(api.updateDocument).not.toHaveBeenCalled();
  });

  it("waits for a push that is already in flight", async () => {
    const { api, useDocumentStore } = await setup();
    const inFlight = deferred<never[]>();
    api.updateDocument.mockReturnValueOnce(inFlight.promise);
    const store = useDocumentStore.getState();

    store.setAtMs(0, 10);
    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS); // push is now in flight

    let flushed = false;
    const flushing = store.flush().then(() => {
      flushed = true;
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(flushed).toBe(false);

    inFlight.resolve([]);
    await flushing;
    expect(flushed).toBe(true);
    expect(api.updateDocument).toHaveBeenCalledTimes(1);
  });

  it("never overlaps pushes, so an older request cannot land after a newer one", async () => {
    const stores = await setup();
    const first = deferred<never[]>();
    stores.api.updateDocument.mockReturnValueOnce(first.promise);
    const store = stores.useDocumentStore.getState();

    store.setAtMs(0, 10);
    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS); // first push in flight

    store.setAtMs(0, 99);
    const flushing = store.flush();
    await vi.advanceTimersByTimeAsync(0);
    expect(stores.api.updateDocument).toHaveBeenCalledTimes(1); // second must wait for the first

    first.resolve([]);
    await flushing;
    expect(stores.api.updateDocument).toHaveBeenCalledTimes(2);
    expect(pushed(stores, 1).timeline[0].at_ms).toBe(99);
  });

  it("retries a failed push on the next flush and reports the failure", async () => {
    const stores = await setup();
    stores.api.updateDocument.mockRejectedValueOnce(new Error("backend down"));
    const store = stores.useDocumentStore.getState();

    store.setAtMs(0, 10);
    await vi.advanceTimersByTimeAsync(PUSH_DEBOUNCE_MS);
    expect(stores.api.updateDocument).toHaveBeenCalledTimes(1);
    expect(stores.useLogStore.getState().logs.some((l) => l.message.includes("Failed to sync edit"))).toBe(true);

    await store.flush();
    expect(stores.api.updateDocument).toHaveBeenCalledTimes(2);
    expect(pushed(stores, 1).timeline[0].at_ms).toBe(10);
  });
});

describe("document replacement", () => {
  it("drops a pending push when the backend replaces the document", async () => {
    const { api, useDocumentStore } = await setup();
    const store = useDocumentStore.getState();

    store.setAtMs(0, 10); // pending
    store.applyDocument(macro("Loaded", [click(7)]));

    await vi.advanceTimersByTimeAsync(5_000);
    expect(api.updateDocument).not.toHaveBeenCalled();
    expect(useDocumentStore.getState().doc.name).toBe("Loaded");
    expect(useDocumentStore.getState().doc.timeline).toHaveLength(1);
  });

  it("refreshFromBackend mirrors the backend's document in one step", async () => {
    const stores = await setup();
    primeBackend(stores.api, { doc: macro("Next", [click(5)]) });

    await stores.useDocumentStore.getState().refreshFromBackend();

    expect(doc(stores).name).toBe("Next");
    expect(doc(stores).timeline).toEqual([click(5)]);
  });

  it("importing YAML replaces the mirror and returns the document", async () => {
    const stores = await setup();
    stores.api.importYaml.mockResolvedValue(macro("Imported"));

    const imported = await stores.useDocumentStore.getState().importYaml("name: Imported");

    expect(imported.name).toBe("Imported");
    expect(doc(stores).name).toBe("Imported");
  });
});

describe("validation findings", () => {
  const finding = { path: "timeline", message: "Timeline must have at least one action", line: null };

  it("surfaces the findings of the last push without blocking the edit", async () => {
    const stores = await setup();
    stores.api.updateDocument.mockResolvedValue([finding]);
    const store = stores.useDocumentStore.getState();

    store.deleteAction(1);
    store.deleteAction(0);
    await store.flush();

    expect(doc(stores).timeline).toEqual([]); // the edit stands
    expect(stores.useDocumentStore.getState().issues).toEqual([finding]);
  });

  it("clears them once a later push is clean", async () => {
    const stores = await setup();
    stores.api.updateDocument.mockResolvedValueOnce([finding]).mockResolvedValue([]);
    const store = stores.useDocumentStore.getState();

    store.setName("");
    await store.flush();
    expect(stores.useDocumentStore.getState().issues).toHaveLength(1);

    store.setName("fixed");
    await store.flush();
    expect(stores.useDocumentStore.getState().issues).toEqual([]);
  });

  it("ignores the findings of a push for a document the backend has since replaced", async () => {
    const stores = await setup();
    const late = deferred<(typeof finding)[]>();
    stores.api.updateDocument.mockReturnValueOnce(late.promise);
    const store = stores.useDocumentStore.getState();

    store.setName("");
    const flushing = store.flush();
    await vi.advanceTimersByTimeAsync(0);
    store.applyDocument(macro("Loaded")); // replaced while the push is in flight
    late.resolve([finding]);
    await flushing;

    expect(stores.useDocumentStore.getState().issues).toEqual([]);
  });
});

describe("lossless editing", () => {
  const sink = fixture as unknown as MacroDocument;

  it("an unrelated edit pushes every other part of a parameterized macro untouched", async () => {
    // The bug this architecture exists to prevent: renaming a macro that uses call_macro,
    // expressions, variables and control flow used to flatten or drop steps.
    const stores = await setup();
    stores.useDocumentStore.getState().applyDocument(structuredClone(sink));

    stores.useDocumentStore.getState().setName("renamed");
    await stores.useDocumentStore.getState().flush();

    expect(pushed(stores)).toEqual({ ...sink, name: "renamed" });
  });

  it("run, target and metadata edits leave the timeline byte-for-byte alone", async () => {
    const stores = await setup();
    stores.useDocumentStore.getState().applyDocument(structuredClone(sink));
    const store = stores.useDocumentStore.getState();

    store.setSpeed(2);
    store.setCountdownSecs(5);
    store.setRepeatText("");
    store.setAuthor("someone else");
    store.setTargetTitle("Calc", "calc.exe");
    await store.flush();

    const sent = pushed(stores);
    expect(sent.timeline).toEqual(sink.timeline);
    expect(sent.variables).toEqual(sink.variables);
    // Editing the run settings keeps the ones the editor does not touch (the fixture has jitter).
    expect(sent.run).toEqual({ repeat: 0, start_delay_ms: 5000, speed: 2, humanize: { position_px: 3, delay_ms: 25 } });
    expect(sent.author).toBe("someone else");
    // The fixture is window-relative; switching windows must not silently turn that off.
    expect(sent.target_window).toEqual({
      title: "Calc",
      process: "calc.exe",
      pause_when_unfocused: false,
      coordinates: "window",
    });
  });

  it("step edits on a parameterized macro keep neighbouring expressions intact", async () => {
    const stores = await setup();
    stores.useDocumentStore.getState().applyDocument(structuredClone(sink));
    const store = stores.useDocumentStore.getState();

    store.adjustDelay(0, 50);
    store.toggleAction(1);
    await store.flush();

    const sent = pushed(stores);
    expect(sent.timeline[0].at_ms).toBe(sink.timeline[0].at_ms + 50);
    expect(sent.timeline[1].enabled).toBe(false);
    expect(sent.timeline.slice(2)).toEqual(sink.timeline.slice(2));
    expect(JSON.stringify(sent.timeline[0].action)).toContain("{{ i * 10 }}");
  });
});

describe("document-level fields", () => {
  it("setTargetTitle binds a window, keeps the pause preference, and clears the binding", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setTargetTitle("Notepad", "notepad.exe");
    expect(doc(stores).target_window).toEqual({ title: "Notepad", process: "notepad.exe", pause_when_unfocused: true });

    store.setPauseWhenUnfocused(false);
    store.setTargetTitle("Calc", "calc.exe");
    expect(doc(stores).target_window?.pause_when_unfocused).toBe(false);

    store.setTargetTitle("", "");
    expect(doc(stores)).not.toHaveProperty("target_window");
  });

  it("setCoordinates marks the document window-relative, keeps that across windows, and clears it for the screen", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setCoordinates("window");
    expect(doc(stores)).not.toHaveProperty("target_window"); // needs a window to belong to

    store.setTargetTitle("Notepad", "notepad.exe");
    store.setCoordinates("window");
    expect(doc(stores).target_window?.coordinates).toBe("window");

    store.setTargetTitle("Calc", "calc.exe");
    expect(doc(stores).target_window).toEqual({
      title: "Calc",
      process: "calc.exe",
      pause_when_unfocused: true,
      coordinates: "window",
    });

    store.setCoordinates("screen");
    expect(doc(stores).target_window).not.toHaveProperty("coordinates");

    store.setCoordinates("window");
    store.setTargetTitle("", "");
    expect(doc(stores)).not.toHaveProperty("target_window");
  });

  it("the pause preference needs a target window to belong to", async () => {
    const stores = await setup();
    stores.useDocumentStore.getState().setPauseWhenUnfocused(false);
    expect(doc(stores)).not.toHaveProperty("target_window");
  });

  it("maps playback fields onto the run config", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setSpeed(0.5);
    store.setRepeatText("3");
    store.setCountdownSecs(7);
    expect(doc(stores).run).toEqual({ repeat: 3, start_delay_ms: 7000, speed: 0.5 });

    store.setRepeatText("");
    store.setCountdownSecs(-4);
    expect(doc(stores).run).toEqual({ repeat: 0, start_delay_ms: 0, speed: 0.5 });
  });

  it("setHumanize sets each variation on its own, clamps it, and removes it when both are zero", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();
    expect(doc(stores).run).not.toHaveProperty("humanize");

    store.setHumanize({ position_px: 4 });
    expect(doc(stores).run.humanize).toEqual({ position_px: 4, delay_ms: 0 });

    store.setHumanize({ delay_ms: 25.6 });
    expect(doc(stores).run.humanize).toEqual({ position_px: 4, delay_ms: 26 });

    // Out of range or not a number: held to what the backend accepts.
    store.setHumanize({ position_px: 5000, delay_ms: -3 });
    expect(doc(stores).run.humanize).toEqual({ position_px: 100, delay_ms: 0 });
    store.setHumanize({ position_px: Number.NaN });
    expect(doc(stores).run.humanize).toBeUndefined();
    expect(doc(stores).run).not.toHaveProperty("humanize");

    // The other run settings are left alone.
    expect(doc(stores).run.speed).toBe(1);
  });
});

describe("timeline mutations", () => {
  it("insertAfter reuses the anchor's time and appendAction the last step's", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.insertAfter(0, { wait: { ms: 5 } });
    expect(doc(stores).timeline.map((t) => t.at_ms)).toEqual([0, 0, 100]);

    store.appendAction({ wait: { ms: 5 } });
    expect(doc(stores).timeline.at(-1)?.at_ms).toBe(100);
  });

  it("clamps retiming at zero, rounds to whole milliseconds, and leaves other steps alone", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.adjustDelay(1, -500);
    store.setAtMs(0, -5);
    expect(doc(stores).timeline.map((t) => t.at_ms)).toEqual([0, 0]);

    store.setAtMs(0, 12.6);
    expect(doc(stores).timeline[0].at_ms).toBe(13);
  });

  it("moveAction swaps neighbours and ignores moves past either end", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.moveAction(0, -1);
    store.moveAction(1, 1);
    expect(doc(stores).timeline.map((t) => t.at_ms)).toEqual([0, 100]);

    store.moveAction(0, 1);
    expect(doc(stores).timeline.map((t) => t.at_ms)).toEqual([100, 0]);
  });

  it("duplicateAction deep-copies so later edits do not alias the original", async () => {
    const stores = await setup();
    stores.useDocumentStore.getState().duplicateAction(0);

    const [a, b] = doc(stores).timeline;
    expect(a.action).toEqual(b.action);
    expect(a.action).not.toBe(b.action);
  });

  it("toggleAction flips a step whose `enabled` is absent (serde omits `true`)", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.toggleAction(0);
    expect(doc(stores).timeline[0].enabled).toBe(false);
    store.toggleAction(0);
    expect(doc(stores).timeline[0].enabled).toBe(true);
  });

  it("setNote stores a blank note as null", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setNote(0, "hello");
    expect(doc(stores).timeline[0].note).toBe("hello");
    store.setNote(0, "");
    expect(doc(stores).timeline[0].note).toBeNull();
  });

  it("deleteAction and setAction change only the targeted step", async () => {
    const stores = await setup();
    stores.useDocumentStore.setState({ doc: macro("M", [click(1), wait(5, 10), click(3, 20)]) });
    const store = stores.useDocumentStore.getState();

    store.setAction(1, { wait: { ms: 99 } });
    store.deleteAction(0);

    expect(doc(stores).timeline).toEqual([wait(99, 10), click(3, 20)]);
  });
});

describe("variable definitions", () => {
  it("setVariables replaces the declared variables and pushes them with the document", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setVariables({ step: { type: "number", default: 10, description: "px per move" } });
    await store.flush();

    expect(pushed(stores).variables).toEqual({ step: { type: "number", default: 10, description: "px per move" } });
    expect(pushed(stores).timeline).toHaveLength(2);
  });

  it("an empty map removes the field, as the backend omits an empty one", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setVariables({ a: { type: "string", default: "" } });
    store.setVariables({});
    await store.flush();

    expect(doc(stores)).not.toHaveProperty("variables");
    expect(JSON.parse(JSON.stringify(pushed(stores)))).not.toHaveProperty("variables");
  });
});

describe("editing control flow and calls", () => {
  it("clearing an optional parameter leaves it out of what the backend receives", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();

    store.setAction(0, {
      wait_until: { condition: "always", timeout_ms: 5000, poll_interval_ms: 100 },
    });
    // The Inspector clears a field by writing `undefined`.
    store.setAction(0, { wait_until: { condition: "always", timeout_ms: undefined, poll_interval_ms: 100 } });
    store.setAction(1, {
      conditional: { condition: "always", then_action: "exit", else_action: { wait: { ms: 1 } } },
    });
    store.setAction(1, { conditional: { condition: "always", then_action: "exit", else_action: undefined } });
    await store.flush();

    const wire = JSON.parse(JSON.stringify(pushed(stores))) as { timeline: { action: Record<string, object> }[] };
    expect(wire.timeline[0].action.wait_until).toEqual({ condition: "always", poll_interval_ms: 100 });
    expect(wire.timeline[1].action.conditional).toEqual({ condition: "always", then_action: "exit" });
  });

  it("a branch edit replaces only that branch and keeps the rest of the step", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();
    const condition = { counter: { key: "n", op: ">=", value: 3 } } as const;

    store.setAction(0, { conditional: { condition, then_action: "exit", else_action: { wait: { ms: 1 } } } });
    const step = doc(stores).timeline[0].action as { conditional: Record<string, unknown> };
    store.setAction(0, { conditional: { ...step.conditional, then_action: { incr_counter: { key: "n" } } } } as never);

    expect(doc(stores).timeline[0].action).toEqual({
      conditional: { condition, then_action: { incr_counter: { key: "n" } }, else_action: { wait: { ms: 1 } } },
    });
  });

  it("a call_macro step round-trips its arguments through edits to other steps", async () => {
    const stores = await setup();
    const store = stores.useDocumentStore.getState();
    const call = { call_macro: { name: "login", args: { user: "{{ who }}", retries: 3, loud: true } } } as const;

    store.setAction(0, call);
    store.setAtMs(1, 500);
    await store.flush();

    expect(pushed(stores).timeline[0].action).toEqual(call);
  });
});
