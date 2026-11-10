import { create } from "zustand";

import { api } from "../lib/ipc";
import type { MouseButton, SimpleActionType } from "../lib/types";
import { useEngineStore } from "./engineStore";
import { log } from "./logStore";
import { usePickStore } from "./pickStore";

/** What `usePickStore` calls the field that asked for the key to capture. */
export const SIMPLE_KEY_FIELD = "simple-key";

/**
 * The Simple tab: one action repeated at a fixed interval. `clickX` / `clickY` are also where
 * Key->Click clicks in its "fixed position" mode, so the picker has one place to put a position.
 */
interface SimpleStore {
  actionType: SimpleActionType;
  clickX: number;
  clickY: number;
  clickButton: MouseButton;
  keyName: string;
  intervalMs: number;
  repeatText: string;
  countdownSecs: number;

  setActionType: (type: SimpleActionType) => void;
  setClickX: (x: number) => void;
  setClickY: (y: number) => void;
  setClickButton: (button: MouseButton) => void;
  setKeyName: (key: string) => void;
  setIntervalMs: (ms: number) => void;
  setRepeatText: (text: string) => void;
  setCountdownSecs: (secs: number) => void;
  setPickedPosition: (x: number, y: number) => void;

  captureKeyName: () => Promise<void>;
  openPicker: () => Promise<void>;
  start: () => Promise<void>;
}

export const useSimpleStore = create<SimpleStore>((set, get) => ({
  actionType: "click",
  clickX: 640,
  clickY: 360,
  clickButton: "Left",
  keyName: "Space",
  intervalMs: 1000,
  repeatText: "",
  countdownSecs: 3,

  setActionType: (actionType) => set({ actionType }),
  setClickX: (clickX) => set({ clickX }),
  setClickY: (clickY) => set({ clickY }),
  setClickButton: (clickButton) => set({ clickButton }),
  setKeyName: (keyName) => set({ keyName }),
  setIntervalMs: (intervalMs) => set({ intervalMs }),
  setRepeatText: (repeatText) => set({ repeatText }),
  setCountdownSecs: (countdownSecs) => set({ countdownSecs }),

  setPickedPosition: (clickX, clickY) => {
    set({ clickX, clickY });
    useEngineStore.getState().setStatus(`Picked: (${clickX}, ${clickY})`);
    log(`Picked: (${clickX}, ${clickY})`);
  },

  captureKeyName: () => usePickStore.getState().captureKey(SIMPLE_KEY_FIELD, get().setKeyName),

  openPicker: async () => {
    // Simple mode asks for this position itself: a request left over from the editor (a picker
    // closed with Esc never reports) must not take it.
    usePickStore.getState().cancelPosition();
    try {
      await api.openPicker();
    } catch (err) {
      log(`Failed to open picker: ${String(err)}`);
    }
  },

  start: async () => {
    const s = get();
    try {
      useEngineStore.getState().resetRunStats();
      await api.startSimpleRepeat({
        action:
          s.actionType === "click"
            ? { type: "click", x: s.clickX, y: s.clickY, button: s.clickButton.toLowerCase() }
            : { type: "key", key: s.keyName },
        intervalMs: s.intervalMs,
        repeatCount: s.repeatText ? parseInt(s.repeatText, 10) : null,
        countdownSecs: s.countdownSecs,
      });
      log("Started");
    } catch (err) {
      useEngineStore.getState().setStatus(`Failed: ${String(err)}`);
      log(`Error: ${String(err)}`);
    }
  },
}));
