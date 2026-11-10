import { create } from "zustand";

import { api, isTauri, pickOpenPath, pickSavePath } from "../lib/ipc";
import type { TemplateInfo } from "../lib/types";
import { useDocumentStore } from "./documentStore";
import { log } from "./logStore";

/** The macros saved on disk and the built-in templates, and the ways to bring one into the editor. */
interface LibraryStore {
  profiles: string[];
  recents: string[];
  templates: TemplateInfo[];

  loadAll: () => Promise<void>;
  loadProfiles: () => Promise<void>;
  loadRecents: () => Promise<void>;
  loadTemplates: () => Promise<void>;
  loadProfile: (name: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  applyTemplate: (id: string) => Promise<void>;
  exportToFile: () => Promise<void>;
  importFromFile: () => Promise<void>;
}

export const useLibraryStore = create<LibraryStore>((set, get) => ({
  profiles: [],
  recents: [],
  templates: [],

  loadAll: async () => {
    await Promise.all([get().loadProfiles(), get().loadRecents(), get().loadTemplates()]);
  },

  loadProfiles: async () => {
    try {
      set({ profiles: await api.listProfiles() });
    } catch (err) {
      log(`Failed to list profiles: ${String(err)}`);
    }
  },

  loadRecents: async () => {
    try {
      set({ recents: await api.getRecentProfiles() });
    } catch (err) {
      log(`Failed to load recent profiles: ${String(err)}`);
    }
  },

  loadTemplates: async () => {
    try {
      set({ templates: await api.listTemplates() });
    } catch (err) {
      log(`Failed to list templates: ${String(err)}`);
    }
  },

  loadProfile: async (name) => {
    try {
      useDocumentStore.getState().applyDocument(await api.loadProfile(name));
      await get().loadRecents();
      log(`Loaded: ${name}`);
    } catch (err) {
      log(`Failed to load ${name}: ${String(err)}`);
    }
  },

  deleteProfile: async (name) => {
    try {
      await api.deleteProfile(name);
      await get().loadProfiles();
      log(`Deleted: ${name}`);
    } catch (err) {
      log(`Failed to delete ${name}: ${String(err)}`);
    }
  },

  applyTemplate: async (id) => {
    try {
      const doc = await api.applyTemplate(id);
      useDocumentStore.getState().applyDocument(doc);
      log(`Applied template: ${doc.name}`);
    } catch (err) {
      log(`Failed to apply template: ${String(err)}`);
    }
  },

  exportToFile: async () => {
    if (!isTauri()) {
      log("Export needs the desktop app (native file dialog).");
      return;
    }
    try {
      const document = useDocumentStore.getState();
      const path = await pickSavePath(`${document.doc.name || "macro"}.yaml`);
      if (!path) return;
      // After the dialog, not before: the export writes what the backend holds, which must
      // include anything edited while the dialog was open.
      await document.flush();
      await api.exportYamlToPath(path);
      log(`Exported to ${path}`);
    } catch (err) {
      log(`Export failed: ${String(err)}`);
    }
  },

  importFromFile: async () => {
    if (!isTauri()) {
      log("Import needs the desktop app (native file dialog).");
      return;
    }
    try {
      const path = await pickOpenPath();
      if (!path) return;
      const doc = await api.importYamlFromPath(path);
      useDocumentStore.getState().applyDocument(doc);
      log(`Imported: ${doc.name}`);
    } catch (err) {
      log(`Import failed: ${String(err)}`);
    }
  },
}));
