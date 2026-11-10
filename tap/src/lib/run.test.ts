import { describe, expect, it, vi } from "vitest";

import { click, deferred, freshStores, macro, primeBackend } from "../test/harness";

vi.mock("./ipc", async () => (await import("../test/ipcMock")).createIpcMock());

/** `run.ts` must be imported *after* `freshStores` resets the module registry. */
async function setup() {
  const stores = await freshStores();
  primeBackend(stores.api);
  stores.api.startExecution.mockResolvedValue(undefined);
  stores.useDocumentStore.setState({ doc: macro("Run", [click(1)]) });
  const { startTimelineRun } = await import("./run");
  return { ...stores, startTimelineRun };
}

describe("startTimelineRun", () => {
  it("force-flushes pending edits to the backend before starting playback", async () => {
    const { api, useDocumentStore, startTimelineRun } = await setup();
    useDocumentStore.getState().setAtMs(0, 123);

    await startTimelineRun();

    expect(api.updateDocument).toHaveBeenCalledTimes(1);
    expect(api.updateDocument.mock.calls[0][0].timeline[0].at_ms).toBe(123);
    expect(api.startExecution).toHaveBeenCalledTimes(1);
    expect(api.updateDocument.mock.invocationCallOrder[0]).toBeLessThan(
      api.startExecution.mock.invocationCallOrder[0],
    );
  });

  it("does not start until an in-flight push has completed", async () => {
    const { api, useDocumentStore, startTimelineRun } = await setup();
    const push = deferred<never[]>();
    api.updateDocument.mockReturnValueOnce(push.promise);
    useDocumentStore.getState().setAtMs(0, 5);

    const running = startTimelineRun();
    await Promise.resolve();
    await Promise.resolve();
    expect(api.startExecution).not.toHaveBeenCalled();

    push.resolve([]);
    await running;
    expect(api.startExecution).toHaveBeenCalledTimes(1);
  });

  it("resets run stats and logs the start", async () => {
    const { useEngineStore, startTimelineRun } = await setup();
    useEngineStore.setState({ executedCount: 9, iteration: 9 });

    await startTimelineRun();

    expect(useEngineStore.getState().executedCount).toBe(0);
    expect(useEngineStore.getState().logs.map((l) => l.message)).toContain("Playing timeline");
  });

  it("reports a start failure in the status line", async () => {
    const { api, useEngineStore, startTimelineRun } = await setup();
    api.startExecution.mockRejectedValue("Cannot start: not in idle state");

    await startTimelineRun();

    expect(useEngineStore.getState().status).toBe("Failed: Cannot start: not in idle state");
  });
});
