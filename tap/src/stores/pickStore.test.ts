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
  const { useToolStore } = await import("./toolStore");
  const { useEngineStore } = await import("./engineStore");
  const { routePickedPosition } = await import("../lib/events");
  return { api: api as unknown as MockApi, usePickStore, useToolStore, useEngineStore, routePickedPosition };
}

let stores: Awaited<ReturnType<typeof load>>;

beforeEach(async () => {
  stores = await load();
});

describe("picking a position for a field", () => {
  it("opens the picker and gives the picked position to the field, not to Simple mode", async () => {
    const { api, usePickStore, useToolStore, routePickedPosition } = stores;
    const picked = vi.fn();
    const before = { x: useToolStore.getState().clickX, y: useToolStore.getState().clickY };

    await usePickStore.getState().requestPosition("field-1", picked);
    expect(api.openPicker).toHaveBeenCalledTimes(1);
    expect(usePickStore.getState().positionTarget).toBe("field-1");

    routePickedPosition(10, 20);

    expect(picked).toHaveBeenCalledWith(10, 20);
    expect(usePickStore.getState().positionTarget).toBeNull();
    expect({ x: useToolStore.getState().clickX, y: useToolStore.getState().clickY }).toEqual(before);
  });

  it("answers a request once: the next position belongs to Simple mode again", async () => {
    const { usePickStore, useToolStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);
    routePickedPosition(1, 2);
    routePickedPosition(30, 40);

    expect(picked).toHaveBeenCalledTimes(1);
    expect([useToolStore.getState().clickX, useToolStore.getState().clickY]).toEqual([30, 40]);
  });

  it("goes to Simple mode when no field asked", () => {
    const { useToolStore, routePickedPosition } = stores;
    routePickedPosition(7, 8);
    expect([useToolStore.getState().clickX, useToolStore.getState().clickY]).toEqual([7, 8]);
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
    const { api, usePickStore, useToolStore, useEngineStore, routePickedPosition } = stores;
    api.openPicker.mockRejectedValueOnce(new Error("no window"));
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked);

    expect(usePickStore.getState().positionTarget).toBeNull();
    expect(useEngineStore.getState().logs.at(-1)?.message).toMatch(/Failed to open picker: Error: no window/);
    routePickedPosition(9, 9);
    expect(picked).not.toHaveBeenCalled();
    expect(useToolStore.getState().clickX).toBe(9);
  });

  it("is dropped when Simple mode opens the picker itself (a picker closed with Esc never reports)", async () => {
    const { usePickStore, useToolStore, routePickedPosition } = stores;
    const picked = vi.fn();

    await usePickStore.getState().requestPosition("field-1", picked); // ...then Esc, no event
    await useToolStore.getState().openPicker();
    routePickedPosition(11, 12);

    expect(picked).not.toHaveBeenCalled();
    expect([useToolStore.getState().clickX, useToolStore.getState().clickY]).toEqual([11, 12]);
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
    const { api, usePickStore, useEngineStore } = stores;
    api.captureKey.mockRejectedValue("Key capture timed out (no key pressed)");
    const captured = vi.fn();

    await usePickStore.getState().captureKey("field-1", captured);

    expect(captured).not.toHaveBeenCalled();
    expect(usePickStore.getState().captureTarget).toBeNull();
    expect(useEngineStore.getState().logs.at(-1)?.message).toMatch(/Key capture failed: .*timed out/);
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
