import { describe, expect, it, vi } from "vitest";

import { click, freshStores, macro } from "../test/harness";

vi.mock("../lib/ipc", async () => (await import("../test/ipcMock")).createIpcMock());

async function setup() {
  const stores = await freshStores();
  const { useLibraryStore } = await import("./libraryStore");
  return { ...stores, useLibraryStore };
}

describe("loading a saved macro", () => {
  it("adopts the loaded document and refreshes the recents", async () => {
    const { api, useDocumentStore, useLibraryStore } = await setup();
    api.loadProfile.mockResolvedValue(macro("Saved", [click(9)]));
    api.getRecentProfiles.mockResolvedValue(["Saved"]);

    await useLibraryStore.getState().loadProfile("Saved");

    expect(useDocumentStore.getState().doc.name).toBe("Saved");
    expect(useLibraryStore.getState().recents).toEqual(["Saved"]);
  });

  it("logs a failure and leaves the document alone", async () => {
    const { api, useDocumentStore, useLibraryStore, useLogStore } = await setup();
    api.loadProfile.mockRejectedValue("not found");
    const before = useDocumentStore.getState().doc;

    await useLibraryStore.getState().loadProfile("Gone");

    expect(useDocumentStore.getState().doc).toBe(before);
    expect(useLogStore.getState().logs.at(-1)?.message).toMatch(/Failed to load Gone/);
  });
});

describe("loadAll", () => {
  it("fills the lists the sidebar shows", async () => {
    const { api, useLibraryStore } = await setup();
    api.listProfiles.mockResolvedValue(["a", "b"]);
    api.getRecentProfiles.mockResolvedValue(["b"]);
    api.listTemplates.mockResolvedValue([{ id: "t", name: "T", description: "" }]);

    await useLibraryStore.getState().loadAll();

    const state = useLibraryStore.getState();
    expect([state.profiles, state.recents, state.templates.map((t) => t.id)]).toEqual([["a", "b"], ["b"], ["t"]]);
  });
});
