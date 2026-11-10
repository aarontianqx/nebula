import { create } from "zustand";

import { emptyDocument, MAX_HUMANIZE_DELAY_MS, MAX_HUMANIZE_POSITION_PX, orUndefined, parseRepeat } from "../lib/document";
import { api, isTauri, pickOpenPath, pickSavePath } from "../lib/ipc";
import type {
  CoordinateSpace,
  DslAction,
  DslTimedAction,
  Humanize,
  MacroDocument,
  TemplateInfo,
  ValidationErrorResponse,
  VariableDefinition,
} from "../lib/types";
import { useEngineStore } from "./engineStore";

const PUSH_DEBOUNCE_MS = 250;

// Debounce/flush bookkeeping kept outside the reactive store.
let pushTimer: ReturnType<typeof setTimeout> | null = null;
let pushInFlight: Promise<void> | null = null;
let needsPush = false;
/** Bumped whenever the backend replaces the document, so a late push result is recognised as stale. */
let generation = 0;

function log(msg: string): void {
  useEngineStore.getState().addLog(msg);
}

interface DocumentStore {
  /**
   * Local mirror of the backend's canonical document. Edits apply here immediately and are pushed
   * (debounced) with `update_document`; the backend never rejects an edit, so the mirror and the
   * document the engine runs only ever differ by the pending push.
   */
  doc: MacroDocument;
  /** Validation findings from the last push (non-blocking; shown inline). */
  issues: ValidationErrorResponse[];
  profiles: string[];
  recents: string[];
  templates: TemplateInfo[];

  // === Backend sync ===
  /** Replace the mirror with a document the backend just produced; drops any pending push. */
  applyDocument: (doc: MacroDocument) => void;
  refreshFromBackend: () => Promise<void>;
  loadProfiles: () => Promise<void>;
  loadRecents: () => Promise<void>;
  loadTemplates: () => Promise<void>;
  loadProfile: (name: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  applyTemplate: (id: string) => Promise<void>;
  importYaml: (yaml: string) => Promise<MacroDocument>;
  exportToFile: () => Promise<void>;
  importFromFile: () => Promise<void>;
  /** Force the pending debounced push to complete (call before start/save/export). */
  flush: () => Promise<void>;

  // === Document-level fields ===
  setName: (name: string) => void;
  setDescription: (value: string) => void;
  setAuthor: (value: string) => void;
  setTags: (tags: string[]) => void;
  setSpeed: (speed: number) => void;
  setRepeatText: (text: string) => void;
  setCountdownSecs: (secs: number) => void;
  /** Change the playback variation; both at zero removes it from the document, as the backend does. */
  setHumanize: (patch: Partial<Humanize>) => void;
  setTargetTitle: (title: string, process: string) => void;
  setPauseWhenUnfocused: (value: boolean) => void;
  /** Measure positions from the target window (`"window"`) or from the screen (the default). */
  setCoordinates: (space: CoordinateSpace) => void;
  /** Replace the declared variables (an empty map removes the field, as the backend omits it). */
  setVariables: (variables: Record<string, VariableDefinition>) => void;

  // === Timeline editing ===
  toggleAction: (index: number) => void;
  deleteAction: (index: number) => void;
  duplicateAction: (index: number) => void;
  moveAction: (index: number, dir: -1 | 1) => void;
  insertAfter: (index: number, action: DslAction) => void;
  appendAction: (action: DslAction) => void;
  setAction: (index: number, action: DslAction) => void;
  setNote: (index: number, note: string) => void;
  setAtMs: (index: number, atMs: number) => void;
  adjustDelay: (index: number, delta: number) => void;
  batchAdjustDelay: (delta: number) => void;
}

export const useDocumentStore = create<DocumentStore>((set, get) => {
  function cancelPendingPush(): void {
    if (pushTimer) {
      clearTimeout(pushTimer);
      pushTimer = null;
    }
    needsPush = false;
  }

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
    // Never overlap pushes: a slow older `update_document` must not be able to land after (and
    // overwrite) a newer one. `pushInFlight` never rejects (failures are handled below).
    while (pushInFlight) await pushInFlight;
    // Snapshot after any wait so the push carries every edit made in the meantime. A failure of
    // the push we just waited on is superseded by this snapshot, so clear its retry mark here.
    needsPush = false;
    const sentIn = generation;
    pushInFlight = api
      .updateDocument(get().doc)
      .then((issues) => {
        // A document the backend replaced meanwhile has its own findings (none, yet).
        if (generation === sentIn) set({ issues });
      })
      .catch((err) => {
        log(`Failed to sync edit: ${String(err)}`);
        needsPush = true;
      })
      .finally(() => {
        pushInFlight = null;
      });
    await pushInFlight;
  }

  /** Apply an edit to the mirror and schedule it for the backend. */
  function mutate(fn: (doc: MacroDocument) => MacroDocument): void {
    set((s) => ({ doc: fn(s.doc) }));
    schedulePush();
  }

  function mutateTimeline(fn: (steps: DslTimedAction[]) => DslTimedAction[]): void {
    mutate((d) => ({ ...d, timeline: fn(d.timeline) }));
  }

  function mapStep(index: number, fn: (step: DslTimedAction) => DslTimedAction): void {
    mutateTimeline((t) => t.map((step, i) => (i === index ? fn(step) : step)));
  }

  function mutateRun(patch: Partial<MacroDocument["run"]>): void {
    mutate((d) => ({ ...d, run: { ...d.run, ...patch } }));
  }

  return {
    doc: emptyDocument(),
    issues: [],
    profiles: [],
    recents: [],
    templates: [],

    applyDocument: (doc) => {
      // A fresh document from the backend replaces the mirror; drop pending pushes.
      cancelPendingPush();
      generation += 1;
      set({ doc, issues: [] });
    },

    refreshFromBackend: async () => {
      try {
        get().applyDocument(await api.getDocument());
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
        get().applyDocument(await api.loadProfile(name));
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
        get().applyDocument(doc);
        log(`Applied template: ${doc.name}`);
      } catch (err) {
        log(`Failed to apply template: ${String(err)}`);
      }
    },

    importYaml: async (yaml) => {
      const doc = await api.importYaml(yaml);
      get().applyDocument(doc);
      log(`Imported: ${doc.name}`);
      return doc;
    },

    exportToFile: async () => {
      if (!isTauri()) {
        log("Export needs the desktop app (native file dialog).");
        return;
      }
      try {
        const path = await pickSavePath(`${get().doc.name || "macro"}.yaml`);
        if (!path) return;
        // After the dialog, not before: the export writes what the backend holds, which must
        // include anything edited while the dialog was open.
        await get().flush();
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
        get().applyDocument(doc);
        log(`Imported: ${doc.name}`);
      } catch (err) {
        log(`Import failed: ${String(err)}`);
      }
    },

    flush: async () => {
      if (needsPush || pushTimer) {
        await pushNow();
      } else if (pushInFlight) {
        await pushInFlight;
      }
    },

    setName: (name) => mutate((d) => ({ ...d, name })),
    setDescription: (value) => mutate((d) => ({ ...d, description: orUndefined(value) })),
    setAuthor: (value) => mutate((d) => ({ ...d, author: orUndefined(value) })),
    setTags: (tags) => mutate((d) => ({ ...d, tags })),

    setSpeed: (speed) => mutateRun({ speed }),
    setRepeatText: (text) => mutateRun({ repeat: parseRepeat(text) }),
    setCountdownSecs: (secs) => mutateRun({ start_delay_ms: Math.max(0, secs) * 1000 }),

    setHumanize: (patch) =>
      mutate((d) => {
        const clamp = (value: number | undefined, max: number, current: number) =>
          value === undefined ? current : Math.min(max, Math.max(0, Math.round(value) || 0));
        const position_px = clamp(patch.position_px, MAX_HUMANIZE_POSITION_PX, d.run.humanize?.position_px ?? 0);
        const delay_ms = clamp(patch.delay_ms, MAX_HUMANIZE_DELAY_MS, d.run.humanize?.delay_ms ?? 0);
        const { humanize: _previous, ...run } = d.run;
        return { ...d, run: position_px === 0 && delay_ms === 0 ? run : { ...run, humanize: { position_px, delay_ms } } };
      }),

    setTargetTitle: (title, process) =>
      mutate((d) => {
        if (title === "" && process === "") {
          // No target any more: drop the binding (and its pause preference) entirely.
          const next = { ...d };
          delete next.target_window;
          return next;
        }
        return {
          ...d,
          target_window: {
            title: title || null,
            process: process || null,
            pause_when_unfocused: d.target_window?.pause_when_unfocused ?? true,
            ...(d.target_window?.coordinates ? { coordinates: d.target_window.coordinates } : {}),
          },
        };
      }),

    setPauseWhenUnfocused: (value) =>
      mutate((d) =>
        d.target_window ? { ...d, target_window: { ...d.target_window, pause_when_unfocused: value } } : d,
      ),

    setCoordinates: (space) =>
      mutate((d) => {
        if (!d.target_window) return d;
        // "screen" is the default and stays out of the document, like on the backend.
        const { coordinates: _previous, ...rest } = d.target_window;
        return { ...d, target_window: space === "window" ? { ...rest, coordinates: space } : rest };
      }),

    setVariables: (variables) =>
      mutate((d) => {
        const next = { ...d };
        if (Object.keys(variables).length === 0) delete next.variables;
        else next.variables = variables;
        return next;
      }),

    toggleAction: (index) => mapStep(index, (a) => ({ ...a, enabled: a.enabled === false })),

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

    setAction: (index, action) => mapStep(index, (a) => ({ ...a, action })),

    setNote: (index, note) => mapStep(index, (a) => ({ ...a, note: note === "" ? null : note })),

    setAtMs: (index, atMs) => mapStep(index, (a) => ({ ...a, at_ms: Math.max(0, Math.round(atMs)) })),

    adjustDelay: (index, delta) => mapStep(index, (a) => ({ ...a, at_ms: Math.max(0, a.at_ms + delta) })),

    batchAdjustDelay: (delta) => mutateTimeline((t) => t.map((a) => ({ ...a, at_ms: Math.max(0, a.at_ms + delta) }))),
  };
});
