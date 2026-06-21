import { create } from "zustand";

import { api, isTauri, pickOpenPath, pickSavePath } from "../lib/ipc";
import type {
  ActionInfo,
  Profile,
  Repeat,
  RunConfig,
  TemplateInfo,
  TimedAction,
  VariableDefinitionResponse,
} from "../lib/types";
import { useEngineStore } from "./engineStore";

const PUSH_DEBOUNCE_MS = 250;
const META_DEBOUNCE_MS = 400;

// Debounce/flush bookkeeping kept outside the reactive store.
let pushTimer: ReturnType<typeof setTimeout> | null = null;
let pushInFlight: Promise<void> | null = null;
let needsPush = false;
let metaTimer: ReturnType<typeof setTimeout> | null = null;

function log(msg: string): void {
  useEngineStore.getState().addLog(msg);
}

interface DocumentStore {
  name: string;
  timeline: TimedAction[];
  run: RunConfig;
  targetTitle: string;
  targetProcess: string;
  pauseWhenUnfocused: boolean;
  variables: VariableDefinitionResponse[];
  /** False when the macro carries variables/expressions; the visual view is then a read-only preview. */
  editable: boolean;
  profiles: string[];
  recents: string[];
  templates: TemplateInfo[];

  // === Document metadata (lossless: safe even for parameterized macros) ===
  description: string;
  author: string;
  tags: string[];

  // === Backend sync ===
  currentProfile: () => Profile;
  applyProfile: (profile: Profile) => void;
  setVariables: (vars: VariableDefinitionResponse[]) => void;
  refreshFromBackend: () => Promise<void>;
  loadProfiles: () => Promise<void>;
  loadRecents: () => Promise<void>;
  loadTemplates: () => Promise<void>;
  loadProfile: (name: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  applyTemplate: (id: string) => Promise<void>;
  importYaml: (yaml: string) => Promise<Profile>;
  exportToFile: () => Promise<void>;
  importFromFile: () => Promise<void>;
  /** Force the pending debounced push to complete (call before start/save). */
  flush: () => Promise<void>;

  // === Metadata edits (debounced, lossless) ===
  loadMeta: () => Promise<void>;
  setDescription: (value: string) => void;
  setAuthor: (value: string) => void;
  setTags: (tags: string[]) => void;

  // === Meta / run config ===
  setName: (name: string) => void;
  setSpeed: (speed: number) => void;
  setRepeatText: (text: string) => void;
  setCountdownSecs: (secs: number) => void;
  setTargetTitle: (title: string, process: string) => void;
  setPauseWhenUnfocused: (value: boolean) => void;

  // === Timeline editing (no-ops unless `editable`) ===
  toggleAction: (index: number) => void;
  deleteAction: (index: number) => void;
  duplicateAction: (index: number) => void;
  moveAction: (index: number, dir: -1 | 1) => void;
  insertAfter: (index: number, action: ActionInfo) => void;
  appendAction: (action: ActionInfo) => void;
  setAction: (index: number, action: ActionInfo) => void;
  setNote: (index: number, note: string) => void;
  setAtMs: (index: number, atMs: number) => void;
  adjustDelay: (index: number, delta: number) => void;
  batchAdjustDelay: (delta: number) => void;
}

function defaultRun(): RunConfig {
  return { start_delay_ms: 3000, speed: 1.0, repeat: "Forever" };
}

export const useDocumentStore = create<DocumentStore>((set, get) => {
  function schedulePush(): void {
    needsPush = true;
    if (pushTimer) clearTimeout(pushTimer);
    pushTimer = setTimeout(() => {
      void pushNow();
    }, PUSH_DEBOUNCE_MS);
  }

  async function pushNow(): Promise<void> {
    if (pushTimer) {
      clearTimeout(pushTimer);
      pushTimer = null;
    }
    needsPush = false;
    const profile = get().currentProfile();
    pushInFlight = api
      .updateProfile(profile)
      .catch((err) => {
        log(`Failed to sync edit: ${String(err)}`);
        needsPush = true;
      })
      .finally(() => {
        pushInFlight = null;
      });
    await pushInFlight;
  }

  function scheduleMetaPush(): void {
    if (metaTimer) clearTimeout(metaTimer);
    metaTimer = setTimeout(() => {
      void pushMetaNow();
    }, META_DEBOUNCE_MS);
  }

  async function pushMetaNow(): Promise<void> {
    if (metaTimer) {
      clearTimeout(metaTimer);
      metaTimer = null;
    }
    const s = get();
    try {
      await api.setDocumentMeta({
        description: s.description.trim() === "" ? null : s.description,
        author: s.author.trim() === "" ? null : s.author,
        tags: s.tags,
      });
    } catch (err) {
      log(`Failed to sync metadata: ${String(err)}`);
    }
  }

  /** Apply a timeline mutation, but only when the document is visual-editable. */
  function mutateTimeline(fn: (actions: TimedAction[]) => TimedAction[]): void {
    if (!get().editable) return;
    set((s) => ({ timeline: fn(s.timeline) }));
    schedulePush();
  }

  // Run/name/target edits also re-push currentProfile(), whose timeline is the
  // (lossy) resolved view for parameterized macros. Gate them on `editable` too
  // so variables/expressions are never flattened by a stray meta edit.
  function mutateMeta(apply: () => void): void {
    if (!get().editable) return;
    apply();
    schedulePush();
  }

  return {
    name: "Untitled",
    timeline: [],
    run: defaultRun(),
    targetTitle: "",
    targetProcess: "",
    pauseWhenUnfocused: true,
    variables: [],
    editable: true,
    profiles: [],
    recents: [],
    templates: [],
    description: "",
    author: "",
    tags: [],

    currentProfile: () => {
      const s = get();
      const hasTarget = s.targetTitle !== "" || s.targetProcess !== "";
      return {
        name: s.name,
        timeline: { actions: s.timeline },
        run: s.run,
        target_window: hasTarget
          ? {
              title: s.targetTitle || null,
              process: s.targetProcess || null,
              pause_when_unfocused: s.pauseWhenUnfocused,
            }
          : null,
      };
    },

    applyProfile: (profile) => {
      // A fresh document from the backend replaces the mirror; drop pending pushes.
      if (pushTimer) {
        clearTimeout(pushTimer);
        pushTimer = null;
      }
      if (metaTimer) {
        clearTimeout(metaTimer);
        metaTimer = null;
      }
      needsPush = false;
      const tw = profile.target_window;
      set({
        name: profile.name,
        timeline: profile.timeline.actions,
        run: profile.run,
        targetTitle: tw?.title ?? "",
        targetProcess: tw?.process ?? "",
        pauseWhenUnfocused: tw?.pause_when_unfocused ?? true,
      });
    },

    setVariables: (variables) => set({ variables, editable: variables.length === 0 }),

    refreshFromBackend: async () => {
      try {
        const [profile, variables] = await Promise.all([api.getCurrentProfile(), api.getMacroVariables()]);
        get().applyProfile(profile);
        get().setVariables(variables);
        await get().loadMeta();
      } catch (err) {
        log(`Failed to load document: ${String(err)}`);
      }
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
        const profile = await api.loadProfile(name);
        get().applyProfile(profile);
        get().setVariables(await api.getMacroVariables());
        await get().loadMeta();
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
        const profile = await api.applyTemplate(id);
        get().applyProfile(profile);
        get().setVariables(await api.getMacroVariables());
        await get().loadMeta();
        log(`Applied template: ${profile.name}`);
      } catch (err) {
        log(`Failed to apply template: ${String(err)}`);
      }
    },

    importYaml: async (yaml) => {
      const profile = await api.importYaml(yaml);
      get().applyProfile(profile);
      get().setVariables(await api.getMacroVariables());
      await get().loadMeta();
      log(`Imported: ${profile.name}`);
      return profile;
    },

    exportToFile: async () => {
      if (!isTauri()) {
        log("Export needs the desktop app (native file dialog).");
        return;
      }
      const s = get();
      try {
        await s.flush();
        const path = await pickSavePath(`${s.name || "macro"}.yaml`);
        if (!path) return;
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
        const profile = await api.importYamlFromPath(path);
        get().applyProfile(profile);
        get().setVariables(await api.getMacroVariables());
        await get().loadMeta();
        log(`Imported: ${profile.name}`);
      } catch (err) {
        log(`Import failed: ${String(err)}`);
      }
    },

    flush: async () => {
      if (metaTimer) {
        await pushMetaNow();
      }
      if (needsPush || pushTimer) {
        await pushNow();
      } else if (pushInFlight) {
        await pushInFlight;
      }
    },

    loadMeta: async () => {
      try {
        const m = await api.getDocumentMeta();
        set({ description: m.description ?? "", author: m.author ?? "", tags: m.tags });
      } catch (err) {
        log(`Failed to load metadata: ${String(err)}`);
      }
    },

    setDescription: (description) => {
      set({ description });
      scheduleMetaPush();
    },
    setAuthor: (author) => {
      set({ author });
      scheduleMetaPush();
    },
    setTags: (tags) => {
      set({ tags });
      scheduleMetaPush();
    },

    setName: (name) => mutateMeta(() => set({ name })),
    setSpeed: (speed) => mutateMeta(() => set((s) => ({ run: { ...s.run, speed } }))),
    setRepeatText: (text) =>
      mutateMeta(() => {
        const trimmed = text.trim();
        const repeat: Repeat = trimmed === "" ? "Forever" : { Times: Math.max(1, parseInt(trimmed, 10) || 1) };
        set((s) => ({ run: { ...s.run, repeat } }));
      }),
    setCountdownSecs: (secs) =>
      mutateMeta(() => set((s) => ({ run: { ...s.run, start_delay_ms: Math.max(0, secs) * 1000 } }))),
    setTargetTitle: (targetTitle, targetProcess) => mutateMeta(() => set({ targetTitle, targetProcess })),
    setPauseWhenUnfocused: (pauseWhenUnfocused) => mutateMeta(() => set({ pauseWhenUnfocused })),

    toggleAction: (index) =>
      mutateTimeline((t) => t.map((a, i) => (i === index ? { ...a, enabled: !a.enabled } : a))),

    deleteAction: (index) => mutateTimeline((t) => t.filter((_, i) => i !== index)),

    duplicateAction: (index) =>
      mutateTimeline((t) => {
        const copy = t[index];
        if (!copy) return t;
        const next = t.slice();
        next.splice(index + 1, 0, { ...copy, action: structuredClone(copy.action) });
        return next;
      }),

    moveAction: (index, dir) =>
      mutateTimeline((t) => {
        const target = index + dir;
        if (target < 0 || target >= t.length) return t;
        const next = t.slice();
        [next[index], next[target]] = [next[target], next[index]];
        return next;
      }),

    insertAfter: (index, action) =>
      mutateTimeline((t) => {
        const at = t[index]?.at_ms ?? 0;
        const next = t.slice();
        next.splice(index + 1, 0, { at_ms: at, action, enabled: true, note: null });
        return next;
      }),

    appendAction: (action) =>
      mutateTimeline((t) => {
        const at = t.length > 0 ? t[t.length - 1].at_ms : 0;
        return [...t, { at_ms: at, action, enabled: true, note: null }];
      }),

    setAction: (index, action) =>
      mutateTimeline((t) => t.map((a, i) => (i === index ? { ...a, action } : a))),

    setNote: (index, note) =>
      mutateTimeline((t) => t.map((a, i) => (i === index ? { ...a, note: note === "" ? null : note } : a))),

    setAtMs: (index, atMs) =>
      mutateTimeline((t) => t.map((a, i) => (i === index ? { ...a, at_ms: Math.max(0, atMs) } : a))),

    adjustDelay: (index, delta) =>
      mutateTimeline((t) => t.map((a, i) => (i === index ? { ...a, at_ms: Math.max(0, a.at_ms + delta) } : a))),

    batchAdjustDelay: (delta) =>
      mutateTimeline((t) => t.map((a) => ({ ...a, at_ms: Math.max(0, a.at_ms + delta) }))),
  };
});
