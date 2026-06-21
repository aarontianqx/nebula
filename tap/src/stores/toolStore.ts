import { create } from "zustand";

import { api } from "../lib/ipc";
import type {
  ColorResponse,
  KeyClickEvent,
  KeyClickLocationMode,
  MouseButton,
  SimpleActionType,
} from "../lib/types";
import { useEngineStore } from "./engineStore";

interface ToolStore {
  // Simple mode
  actionType: SimpleActionType;
  clickX: number;
  clickY: number;
  clickButton: MouseButton;
  keyName: string;
  capturingKey: boolean;
  intervalMs: number;
  repeatText: string;
  countdownSecs: number;

  // Key-to-Click
  keyClickRunning: boolean;
  keyClickCount: number;
  keyClickInterval: number;
  keyClickHoldDelay: number;
  keyClickButton: MouseButton;
  keyClickLocationMode: KeyClickLocationMode;
  keyClickOnlyTargetFocused: boolean;

  // Color sampling
  pickedColor: ColorResponse | null;

  setActionType: (type: SimpleActionType) => void;
  setClickX: (x: number) => void;
  setClickY: (y: number) => void;
  setClickButton: (button: MouseButton) => void;
  setKeyName: (key: string) => void;
  captureKeyName: () => Promise<void>;
  setIntervalMs: (ms: number) => void;
  setRepeatText: (text: string) => void;
  setCountdownSecs: (secs: number) => void;
  setKeyClickInterval: (ms: number) => void;
  setKeyClickHoldDelay: (ms: number) => void;
  setKeyClickButton: (button: MouseButton) => void;
  setKeyClickLocationMode: (mode: KeyClickLocationMode) => void;
  setKeyClickOnlyTargetFocused: (value: boolean) => void;
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
  clickButton: "Left",
  keyName: "Space",
  capturingKey: false,
  intervalMs: 1000,
  repeatText: "",
  countdownSecs: 3,

  keyClickRunning: false,
  keyClickCount: 0,
  keyClickInterval: 40,
  keyClickHoldDelay: 150,
  keyClickButton: "Left",
  keyClickLocationMode: "cursor",
  keyClickOnlyTargetFocused: false,

  pickedColor: null,

  setActionType: (actionType) => set({ actionType }),
  setClickX: (clickX) => set({ clickX }),
  setClickY: (clickY) => set({ clickY }),
  setClickButton: (clickButton) => set({ clickButton }),
  setKeyName: (keyName) => set({ keyName }),

  captureKeyName: async () => {
    if (get().capturingKey) return;
    set({ capturingKey: true });
    log("Press any key to capture...");
    try {
      const key = await api.captureKey();
      set({ keyName: key });
      log(`Captured key: ${key}`);
    } catch (err) {
      log(`Key capture failed: ${String(err)}`);
    } finally {
      set({ capturingKey: false });
    }
  },
  setIntervalMs: (intervalMs) => set({ intervalMs }),
  setRepeatText: (repeatText) => set({ repeatText }),
  setCountdownSecs: (countdownSecs) => set({ countdownSecs }),
  setKeyClickInterval: (keyClickInterval) => set({ keyClickInterval }),
  setKeyClickHoldDelay: (keyClickHoldDelay) => set({ keyClickHoldDelay }),
  setKeyClickButton: (keyClickButton) => set({ keyClickButton }),
  setKeyClickLocationMode: (keyClickLocationMode) => set({ keyClickLocationMode }),
  setKeyClickOnlyTargetFocused: (keyClickOnlyTargetFocused) => set({ keyClickOnlyTargetFocused }),

  setPickedPosition: (clickX, clickY) => {
    set({ clickX, clickY });
    useEngineStore.getState().setUiMessage(`Picked: (${clickX}, ${clickY})`);
    log(`Picked: (${clickX}, ${clickY})`);
  },

  handleKeyClickEvent: (e) => {
    if (e === "Started") {
      set({ keyClickRunning: true, keyClickCount: 0 });
      log("Key->Click active - hold A-Z to click, Space to stop");
    } else if (typeof e === "object" && "Click" in e) {
      set({ keyClickCount: e.Click.count });
    } else if (typeof e === "object" && "Stopped" in e) {
      set({ keyClickRunning: false });
      const how = e.Stopped.reason === "space" ? "Space" : "Stop";
      log(`Key->Click stopped by ${how} (${e.Stopped.total_clicks} clicks)`);
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
        button: s.actionType === "click" ? s.clickButton.toLowerCase() : null,
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
      const button = s.keyClickButton.toLowerCase() as "left" | "right" | "middle";
      const location =
        s.keyClickLocationMode === "fixed" ? `Fixed (${s.clickX}, ${s.clickY})` : "Cursor";
      log(
        `Key->Click: button=${s.keyClickButton}, location=${location}, min interval=${s.keyClickInterval}ms` +
          (s.keyClickOnlyTargetFocused ? ", locked to active window" : ""),
      );
      await api.startKeyClick({
        minIntervalMs: s.keyClickInterval,
        holdDelayMs: s.keyClickHoldDelay,
        button,
        locationMode: s.keyClickLocationMode,
        fixedX: s.clickX,
        fixedY: s.clickY,
        onlyTargetFocused: s.keyClickOnlyTargetFocused,
      });
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
