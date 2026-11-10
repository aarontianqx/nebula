import { create } from "zustand";

import { emptyDocument, MAX_HUMANIZE_DELAY_MS, MAX_HUMANIZE_POSITION_PX, orUndefined, parseRepeat } from "../lib/document";
import { DebouncedPush } from "../lib/debouncedPush";
import { api } from "../lib/ipc";
import type {
  CoordinateSpace,
  DslAction,
  DslTimedAction,
  Humanize,
  MacroDocument,
  ValidationErrorResponse,
  VariableDefinition,
} from "../lib/types";
import { log } from "./logStore";

const PUSH_DEBOUNCE_MS = 250;

interface DocumentStore {
  /**
   * Local mirror of the backend's canonical document. Edits apply here immediately and are pushed
   * (debounced) with `update_document`; the backend never rejects an edit, so the mirror and the
   * document the engine runs only ever differ by the pending push.
   */
  doc: MacroDocument;
  /** Validation findings from the last push (non-blocking; shown inline). */
  issues: ValidationErrorResponse[];

  // === Backend sync ===
  /** Replace the mirror with a document the backend just produced; drops any pending push. */
  applyDocument: (doc: MacroDocument) => void;
  refreshFromBackend: () => Promise<void>;
  importYaml: (yaml: string) => Promise<MacroDocument>;
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
  const sync = new DebouncedPush({
    delayMs: PUSH_DEBOUNCE_MS,
    send: () => api.updateDocument(get().doc),
    onResult: (issues) => set({ issues }),
    onError: (err) => log(`Failed to sync edit: ${String(err)}`),
  });

  /** Apply an edit to the mirror and schedule it for the backend. */
  function mutate(fn: (doc: MacroDocument) => MacroDocument): void {
    set((s) => ({ doc: fn(s.doc) }));
    sync.schedule();
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

    applyDocument: (doc) => {
      sync.discard();
      set({ doc, issues: [] });
    },

    refreshFromBackend: async () => {
      try {
        get().applyDocument(await api.getDocument());
      } catch (err) {
        log(`Failed to load document: ${String(err)}`);
      }
    },

    importYaml: async (yaml) => {
      const doc = await api.importYaml(yaml);
      get().applyDocument(doc);
      log(`Imported: ${doc.name}`);
      return doc;
    },

    flush: () => sync.flush(),

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
