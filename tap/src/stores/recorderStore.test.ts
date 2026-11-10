import { describe, expect, it, vi } from "vitest";

import { click, freshStores, macro, primeBackend } from "../test/harness";

vi.mock("../lib/ipc", async () => (await import("../test/ipcMock")).createIpcMock());

async function setup() {
  const stores = await freshStores();
  primeBackend(stores.api);
  stores.api.startRecording.mockResolvedValue(undefined);
  stores.useDocumentStore.setState({ doc: macro("Rec", [click(1)]) });
  const { useRecorderStore } = await import("./recorderStore");
  return { ...stores, useRecorderStore };
}

describe("starting a recording", () => {
  it("pushes pending edits first: the backend decides, from its document, how positions are recorded", async () => {
    const { api, useDocumentStore, useRecorderStore } = await setup();
    useDocumentStore.getState().setTargetTitle("Game", "game.exe");
    useDocumentStore.getState().setCoordinates("window");

    await useRecorderStore.getState().start();

    expect(api.updateDocument).toHaveBeenCalledTimes(1);
    expect(api.updateDocument.mock.calls[0][0].target_window.coordinates).toBe("window");
    expect(api.updateDocument.mock.invocationCallOrder[0]).toBeLessThan(
      api.startRecording.mock.invocationCallOrder[0],
    );
    expect(useRecorderStore.getState().recorderState).toBe("Recording");
  });

  it("reports a refusal (for example, the target window is not on screen) and stays idle", async () => {
    const { api, useEngineStore, useRecorderStore } = await setup();
    api.startRecording.mockRejectedValue("The target window was not found.");

    await useRecorderStore.getState().start();

    expect(useRecorderStore.getState().recorderState).toBe("Idle");
    expect(useEngineStore.getState().logs.at(-1)?.message).toMatch(/target window was not found/);
  });
});
