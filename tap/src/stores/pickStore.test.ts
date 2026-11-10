// Picking a position / capturing a key for a field of the editor must never disturb Simple mode,
// which listens to the same picker event.

import { beforeEach, describe, expect, it, vi } from "vitest";

import { deferred } from "../test/harness";
import type { MockApi } from "../test/ipcMock";

vi.mock("../lib/ipc", async () => (await import("../test/ipcMock")).createIpcMock());

async function load() {
  vi.resetModules();
  const { api } = await import("../lib/ipc");
  const { usePickStore } = await import("./pickStore");
  const { useSimpleStore } = await import("./simpleStore");
  const { useLogStore } = await import("./logStore");
  const { routePickedPosition } = await import("../lib/events");
  return { api: api as unknown as MockApi, usePickStore, useSimpleStore, useLogStore, routePickedPosition };
}

let stores: Awaited<ReturnType<typeof load>>;

beforeEach(async () => {
  stores = await load();
});

describe("picking a position for a field", () => {
  it("opens the picker and gives the picked position to the field, not to Simple mode", async () => {
    const { api, usePickStore, useSimpleStore, routePickedPosition } = stores;
    const picked = vi.fn();
    const before = { x: useSimpleStore.getState().clickX, y: useSimpleStore.getState().clickY };

    await usePickStore.getState().requestPosition("field-1", picked);
    expect(api.openPicker).toHaveBeenCalledTimes(1);
    expect(usePickStore.getState().positionTarget).toBe("field-1");

    routePickedPosition(10, 20);

    expect(picked).toHaveBeenCalledWith(10, 20);
    expect(usePickStore.getState().positionTarget).toBeNull();
    expect({ x: useSimpleStore.getState().clickX, y: useSimpleStore.getState().clickY }).toEqual(before);
  });

  it("answers a request once: the next position belongs to Simple mode again", async () => {
    const { usePickStore, useSimpleStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);
    routePickedPosition(1, 2);
    routePickedPosition(30, 40);

    expect(picked).toHaveBeenCalledTimes(1);
    expect([useSimpleStore.getState().clickX, useSimpleStore.getState().clickY]).toEqual([30, 40]);
  });

  it("goes to Simple mode when no field asked", () => {
    const { useSimpleStore, routePickedPosition } = stores;
    routePickedPosition(7, 8);
    expect([useSimpleStore.getState().clickX, useSimpleStore.getState().clickY]).toEqual([7, 8]);
  });

  it("serves only the newest request", async () => {
    const { usePickStore, routePickedPosition } = stores;
    const first = vi.fn();
    const second = vi.fn();

    await usePickStore.getState().requestPosition("a", first);
    await usePickStore.getState().requestPosition("b", second);
    routePickedPosition(5, 6);

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith(5, 6);
  });

  it("forgets the request when the picker cannot open", async () => {
    const { api, usePickStore, useSimpleStore, useLogStore, routePickedPosition } = stores;
    api.openPicker.mockRejectedValueOnce(new Error("no window"));
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);

    expect(usePickStore.getState().positionTarget).toBeNull();
    expect(useLogStore.getState().logs.at(-1)?.message).toMatch(/Failed to open picker: Error: no window/);
    routePickedPosition(9, 9);
    expect(picked).not.toHaveBeenCalled();
    expect(useSimpleStore.getState().clickX).toBe(9);
  });

  it("is dropped when Simple mode opens the picker itself (a picker closed with Esc never reports)", async () => {
    const { usePickStore, useSimpleStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked); // ...then Esc, no event
    await useSimpleStore.getState().openPicker();
    routePickedPosition(11, 12);

    expect(picked).not.toHaveBeenCalled();
    expect([useSimpleStore.getState().clickX, useSimpleStore.getState().clickY]).toEqual([11, 12]);
  });

  it("is dropped by cancelAll (the field that asked is gone)", async () => {
    const { usePickStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);
    usePickStore.getState().cancelAll();
    routePickedPosition(1, 1);

    expect(picked).not.toHaveBeenCalled();
  });
});

describe("picking in a document measured from its target window", () => {
  const orange = { r: 255, g: 136, b: 0, hex: "#FF8800" };

  it("hands the field the position relative to the window, with the color untouched", async () => {
    const { usePickStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked, { color: true });
    routePickedPosition(110, 70, orange, { window_origin: { x: 100, y: 50 } });

    expect(picked).toHaveBeenCalledWith(10, 20, orange);
  });

  it("can produce a negative position for a click left of or above the window", async () => {
    const { usePickStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);
    routePickedPosition(90, 40, undefined, { window_origin: { x: 100, y: 50 } });

    expect(picked).toHaveBeenCalledWith(-10, -10);
  });

  it("sets nothing and says so when the target window was not found", async () => {
    const { usePickStore, useLogStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);
    routePickedPosition(110, 70, undefined, { window_missing: true });

    expect(picked).not.toHaveBeenCalled();
    expect(usePickStore.getState().positionTarget).toBeNull();
    expect(useLogStore.getState().logs.at(-1)?.message).toContain("target window was not found");
  });

  it("leaves Simple mode on screen coordinates", () => {
    const { useSimpleStore, routePickedPosition } = stores;
    routePickedPosition(110, 70, undefined, { window_origin: { x: 100, y: 50 } });
    expect([useSimpleStore.getState().clickX, useSimpleStore.getState().clickY]).toEqual([110, 70]);
  });
});

describe("picking a position together with its color", () => {
  const orange = { r: 255, g: 136, b: 0, hex: "#FF8800" };

  it("asks the picker for the color and hands it to the field", async () => {
    const { api, usePickStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked, { color: true });
    expect(api.openPicker).toHaveBeenCalledWith("position_color");

    routePickedPosition(10, 20, orange);
    expect(picked).toHaveBeenCalledWith(10, 20, orange);
  });

  it("asks for the position alone unless a color is wanted", async () => {
    const { api, usePickStore } = stores;

    await usePickStore.getState().requestPosition("field-1", vi.fn());
    expect(api.openPicker).toHaveBeenLastCalledWith("position");

    await usePickStore.getState().requestPosition("field-1", vi.fn(), { color: false });
    expect(api.openPicker).toHaveBeenLastCalledWith("position");
  });

  it("still sets the position, and says so, when the color could not be read", async () => {
    const { usePickStore, useLogStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked, { color: true });
    routePickedPosition(10, 20, undefined);

    expect(picked).toHaveBeenCalledWith(10, 20);
    expect(useLogStore.getState().logs.at(-1)?.message).toMatch(/Could not read the color at \(10, 20\)/);
  });

  it("does not complain about a missing color when none was asked for", async () => {
    const { usePickStore, useLogStore, routePickedPosition } = stores;
    await usePickStore.getState().requestPosition("field-1", vi.fn());
    const logsBefore = useLogStore.getState().logs.length;

    routePickedPosition(1, 2, undefined);

    expect(useLogStore.getState().logs).toHaveLength(logsBefore);
  });

  it("is forgotten with the request: the next plain pick does not expect a color", async () => {
    const { usePickStore, useLogStore, routePickedPosition } = stores;

    await usePickStore.getState().requestPosition("field-1", vi.fn(), { color: true });
    usePickStore.getState().cancelPosition();
    await usePickStore.getState().requestPosition("field-2", vi.fn());
    const logsBefore = useLogStore.getState().logs.length;
    routePickedPosition(5, 6, undefined);

    expect(useLogStore.getState().logs).toHaveLength(logsBefore);
  });

  it("is ignored by Simple mode, which opens the picker for the position alone", async () => {
    const { api, useSimpleStore, routePickedPosition } = stores;

    await useSimpleStore.getState().openPicker();
    expect(api.openPicker).toHaveBeenLastCalledWith();

    routePickedPosition(30, 40, orange);
    expect([useSimpleStore.getState().clickX, useSimpleStore.getState().clickY]).toEqual([30, 40]);
  });
});

describe("capturing a key for a field", () => {
  it("hands the captured key to the field and clears its waiting state", async () => {
    const { api, usePickStore } = stores;
    api.captureKey.mockResolvedValue("F5");
    const captured = vi.fn();

    await usePickStore.getState().captureKey("field-1", captured);

    expect(captured).toHaveBeenCalledWith("F5");
    expect(usePickStore.getState().captureTarget).toBeNull();
  });

  it("shows which field is waiting while the key is awaited", async () => {
    const { api, usePickStore } = stores;
    const pending = deferred<string>();
    api.captureKey.mockReturnValue(pending.promise);

    const running = usePickStore.getState().captureKey("field-1", vi.fn());
    expect(usePickStore.getState().captureTarget).toBe("field-1");

    pending.resolve("A");
    await running;
    expect(usePickStore.getState().captureTarget).toBeNull();
  });

  it("ignores a second capture while one is waiting", async () => {
    const { api, usePickStore } = stores;
    const pending = deferred<string>();
    api.captureKey.mockReturnValue(pending.promise);

    const first = usePickStore.getState().captureKey("a", vi.fn());
    await usePickStore.getState().captureKey("b", vi.fn());
    expect(api.captureKey).toHaveBeenCalledTimes(1);

    pending.resolve("A");
    await first;
  });

  it("logs a failure (timeout, busy engine) and leaves the field alone", async () => {
    const { api, usePickStore, useLogStore } = stores;
    api.captureKey.mockRejectedValue("Key capture timed out (no key pressed)");
    const captured = vi.fn();

    await usePickStore.getState().captureKey("field-1", captured);

    expect(captured).not.toHaveBeenCalled();
    expect(usePickStore.getState().captureTarget).toBeNull();
    expect(useLogStore.getState().logs.at(-1)?.message).toMatch(/Key capture failed: .*timed out/);
  });

  it("ignores the key when the field that asked is gone, and lets the next capture run", async () => {
    const { api, usePickStore } = stores;
    const pending = deferred<string>();
    api.captureKey.mockReturnValueOnce(pending.promise);
    const captured = vi.fn();

    const running = usePickStore.getState().captureKey("old", captured);
    usePickStore.getState().cancelAll();
    expect(usePickStore.getState().captureTarget).toBeNull();

    pending.resolve("A");
    await running;
    expect(captured).not.toHaveBeenCalled();

    api.captureKey.mockResolvedValueOnce("B");
    const next = vi.fn();
    await usePickStore.getState().captureKey("new", next);
    expect(next).toHaveBeenCalledWith("B");
  });

  it("cancelling a capture leaves a pending position request alone", async () => {
    const { api, usePickStore, routePickedPosition } = stores;
    api.captureKey.mockReturnValue(deferred<string>().promise);
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("pos", picked);
    void usePickStore.getState().captureKey("key", vi.fn());
    usePickStore.getState().cancelCapture();
    routePickedPosition(3, 4);

    expect(usePickStore.getState().captureTarget).toBeNull();
    expect(picked).toHaveBeenCalledWith(3, 4);
  });
});
