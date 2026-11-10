import { create } from "zustand";

import { api } from "../lib/ipc";
import type { ColorResponse, PickedWindow } from "../lib/types";
import { log } from "./logStore";

// Picking a position and capturing a key are one-shot requests made by a field of the editor.
// The picker window reports through a global event, so the store remembers who asked: the
// position goes to that field first and only otherwise to Simple mode (which owns the same
// event). Without this, an Inspector pick would overwrite Simple mode's click position.

type PositionConsumer = (x: number, y: number, color?: ColorResponse) => void;
type KeyConsumer = (key: string) => void;

// Callbacks are not state: nothing renders from them.
let positionConsumer: PositionConsumer | null = null;
/** Whether the pending position request also asked for the color under the click. */
let positionWantsColor = false;
/** Bumped to abandon a capture in flight: its result is then ignored. */
let captureGeneration = 0;

interface PickStore {
  /** Identifies the field waiting for a picked position, for its "picking" state. */
  positionTarget: string | null;
  /** Identifies the field waiting for a key press. */
  captureTarget: string | null;

  /**
   * Open the picker; its result goes to `onPicked` instead of Simple mode. With `color`, the
   * color under the click is read as well (and passed on when it could be read).
   */
  requestPosition: (target: string, onPicked: PositionConsumer, options?: { color?: boolean }) => Promise<void>;
  /** Forget a pending request (the next picked position then belongs to Simple mode). */
  cancelPosition: () => void;
  /**
   * Give a picked screen position to whoever asked for it; `false` if nobody did. If the document
   * measures from its target window the position is handed over relative to that window; when the
   * window cannot be found nothing is handed over.
   */
  deliverPosition: (x: number, y: number, color?: ColorResponse, window?: PickedWindow) => boolean;

  /** Wait for the next physical key press and hand its normalized name to `onCaptured`. */
  captureKey: (target: string, onCaptured: KeyConsumer) => Promise<void>;
  /** Abandon a pending capture (the field that asked is gone). */
  cancelCapture: () => void;
  /** Abandon whatever is pending. */
  cancelAll: () => void;
}

export const usePickStore = create<PickStore>((set, get) => ({
  positionTarget: null,
  captureTarget: null,

  requestPosition: async (target, onPicked, options) => {
    positionConsumer = onPicked;
    positionWantsColor = options?.color === true;
    set({ positionTarget: target });
    try {
      await api.openPicker(positionWantsColor ? "position_color" : "position");
    } catch (err) {
      get().cancelPosition();
      log(`Failed to open picker: ${String(err)}`);
    }
  },

  cancelPosition: () => {
    positionConsumer = null;
    positionWantsColor = false;
    set({ positionTarget: null });
  },

  deliverPosition: (x, y, color, window) => {
    const consumer = positionConsumer;
    if (!consumer) return false;
    const wantedColor = positionWantsColor;
    get().cancelPosition();
    if (window?.window_missing) {
      log(`Picked (${x}, ${y}) on screen, but the target window was not found, so no window-relative position was set.`);
      return true;
    }
    if (wantedColor && !color) {
      log(`Could not read the color at (${x}, ${y}); only the position was set. Screen Recording permission may be missing.`);
    }
    const origin = window?.window_origin;
    const px = origin ? x - origin.x : x;
    const py = origin ? y - origin.y : y;
    // Without a color the call keeps its two-argument shape.
    if (color) consumer(px, py, color);
    else consumer(px, py);
    return true;
  },

  captureKey: async (target, onCaptured) => {
    if (get().captureTarget !== null) return;
    const generation = ++captureGeneration;
    set({ captureTarget: target });
    log("Press any key to capture...");
    try {
      const key = await api.captureKey();
      if (generation !== captureGeneration) return;
      onCaptured(key);
      log(`Captured key: ${key}`);
    } catch (err) {
      if (generation === captureGeneration) log(`Key capture failed: ${String(err)}`);
    } finally {
      if (generation === captureGeneration) set({ captureTarget: null });
    }
  },

  cancelCapture: () => {
    captureGeneration += 1;
    set({ captureTarget: null });
  },

  cancelAll: () => {
    get().cancelPosition();
    get().cancelCapture();
  },
}));
