import { create } from "zustand";

import { api } from "../lib/ipc";
import type { ColorResponse, KeyClickEvent, SimpleActionType } from "../lib/types";
import { useEngineStore } from "./engineStore";

interface ToolStore {
  // Simple mode
  actionType: SimpleActionType;
  clickX: number;
  clickY: number;
  keyName: string;
  intervalMs: number;
  repeatText: string;
  countdownSecs: number;

  // Key-to-Click
  keyClickRunning: boolean;
  keyClickCount: number;
  keyClickInterval: number;
  keyClickHoldDelay: number;

  // Color sampling
  pickedColor: ColorResponse | null;

  setActionType: (type: SimpleActionType) => void;
  setClickX: (x: number) => void;
  setClickY: (y: number) => void;
  setKeyName: (key: string) => void;
  setIntervalMs: (ms: number) => void;
  setRepeatText: (text: string) => void;
  setCountdownSecs: (secs: number) => void;
  setKeyClickInterval: (ms: number) => void;
  setKeyClickHoldDelay: (ms: number) => void;
  setPickedPosition: (x: number, y: number) => void;
  handleKeyClickEvent: (event: KeyClickEvent) => void;

  startSimple: () => Promise<void>;
  startKeyClick: () => Promise<void>;
  stopKeyClick: () => Promise<void>;
  openPicker: () => Promise<void>;
  pickColorAtCursor: () => Promise<void>;
}

function log(msg: string): void {
  useEngineStore.getState().addLog(msg);
}

export const useToolStore = create<ToolStore>((set, get) => ({
  actionType: "click",
  clickX: 640,
  clickY: 360,
  keyName: "Space",
  intervalMs: 1000,
  repeatText: "",
  countdownSecs: 3,

  keyClickRunning: false,
  keyClickCount: 0,
  keyClickInterval: 50,
  keyClickHoldDelay: 150,

  pickedColor: null,

  setActionType: (actionType) => set({ actionType }),
  setClickX: (clickX) => set({ clickX }),
  setClickY: (clickY) => set({ clickY }),
  setKeyName: (keyName) => set({ keyName }),
  setIntervalMs: (intervalMs) => set({ intervalMs }),
  setRepeatText: (repeatText) => set({ repeatText }),
  setCountdownSecs: (countdownSecs) => set({ countdownSecs }),
  setKeyClickInterval: (keyClickInterval) => set({ keyClickInterval }),
  setKeyClickHoldDelay: (keyClickHoldDelay) => set({ keyClickHoldDelay }),

  setPickedPosition: (clickX, clickY) => {
    set({ clickX, clickY });
    useEngineStore.getState().setUiMessage(`Picked: (${clickX}, ${clickY})`);
    log(`Picked: (${clickX}, ${clickY})`);
  },

  handleKeyClickEvent: (e) => {
    if (e === "Started") {
      set({ keyClickRunning: true, keyClickCount: 0 });
      log("Key->Click mode started");
    } else if (typeof e === "object" && "Click" in e) {
      set({ keyClickCount: e.Click.count });
    } else if (typeof e === "object" && "Stopped" in e) {
      set({ keyClickRunning: false });
      log(`Key->Click stopped (${e.Stopped.total_clicks} clicks)`);
    }
  },

  startSimple: async () => {
    const s = get();
    try {
      await api.setSimpleRepeat({
        actionType: s.actionType,
        x: s.actionType === "click" ? s.clickX : null,
        y: s.actionType === "click" ? s.clickY : null,
        key: s.actionType === "key" ? s.keyName : null,
        intervalMs: s.intervalMs,
        repeatCount: s.repeatText ? parseInt(s.repeatText, 10) : null,
        countdownSecs: s.countdownSecs,
      });
      useEngineStore.getState().resetRunStats();
      await api.startExecution();
      log("Started");
    } catch (err) {
      useEngineStore.getState().setStatus(`Failed: ${String(err)}`);
      log(`Error: ${String(err)}`);
    }
  },

  startKeyClick: async () => {
    const s = get();
    try {
      set({ keyClickCount: 0 });
      await api.startKeyClick(s.keyClickInterval, s.keyClickHoldDelay);
      log("Key->Click mode starting...");
    } catch (err) {
      useEngineStore.getState().setStatus(`Failed: ${String(err)}`);
      log(`Error: ${String(err)}`);
    }
  },

  stopKeyClick: async () => {
    try {
      await api.stopKeyClick();
    } catch (err) {
      useEngineStore.getState().setStatus(`Failed: ${String(err)}`);
    }
  },

  openPicker: async () => {
    try {
      await api.openPicker();
    } catch (err) {
      log(`Failed to open picker: ${String(err)}`);
    }
  },

  pickColorAtCursor: async () => {
    const pos = useEngineStore.getState().mousePos;
    if (!pos) return;
    try {
      const color = await api.getPixelColor(pos.x, pos.y);
      if (color) {
        set({ pickedColor: color });
        log(`Color at (${pos.x}, ${pos.y}): ${color.hex}`);
      }
    } catch (err) {
      log(`Failed to read color: ${String(err)}`);
    }
  },
}));
