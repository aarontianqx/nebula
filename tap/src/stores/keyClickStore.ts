import { create } from "zustand";

import { api } from "../lib/ipc";
import type { KeyClickEvent, KeyClickLocationMode, MouseButton } from "../lib/types";
import { useEngineStore } from "./engineStore";
import { log } from "./logStore";
import { useSimpleStore } from "./simpleStore";

/** The Key->Click tool: hold a letter to keep clicking, Space to stop. */
interface KeyClickStore {
  running: boolean;
  count: number;
  intervalMs: number;
  holdDelayMs: number;
  button: MouseButton;
  locationMode: KeyClickLocationMode;
  onlyTargetFocused: boolean;

  setIntervalMs: (ms: number) => void;
  setHoldDelayMs: (ms: number) => void;
  setButton: (button: MouseButton) => void;
  setLocationMode: (mode: KeyClickLocationMode) => void;
  setOnlyTargetFocused: (value: boolean) => void;

  handleEvent: (event: KeyClickEvent) => void;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}

export const useKeyClickStore = create<KeyClickStore>((set, get) => ({
  running: false,
  count: 0,
  intervalMs: 40,
  holdDelayMs: 150,
  button: "Left",
  locationMode: "cursor",
  onlyTargetFocused: false,

  setIntervalMs: (intervalMs) => set({ intervalMs }),
  setHoldDelayMs: (holdDelayMs) => set({ holdDelayMs }),
  setButton: (button) => set({ button }),
  setLocationMode: (locationMode) => set({ locationMode }),
  setOnlyTargetFocused: (onlyTargetFocused) => set({ onlyTargetFocused }),

  handleEvent: (e) => {
    if (e === "Started") {
      set({ running: true, count: 0 });
      log("Key->Click active - hold A-Z to click, Space to stop");
    } else if (typeof e === "object" && "Click" in e) {
      set({ count: e.Click.count });
    } else if (typeof e === "object" && "Stopped" in e) {
      set({ running: false });
      const how = e.Stopped.reason === "space" ? "Space" : "Stop";
      log(`Key->Click stopped by ${how} (${e.Stopped.total_clicks} clicks)`);
    }
  },

  start: async () => {
    const s = get();
    const { clickX, clickY } = useSimpleStore.getState();
    try {
      set({ count: 0 });
      const location = s.locationMode === "fixed" ? `Fixed (${clickX}, ${clickY})` : "Cursor";
      log(
        `Key->Click: button=${s.button}, location=${location}, min interval=${s.intervalMs}ms` +
          (s.onlyTargetFocused ? ", locked to active window" : ""),
      );
      await api.startKeyClick({
        minIntervalMs: s.intervalMs,
        holdDelayMs: s.holdDelayMs,
        button: s.button.toLowerCase() as "left" | "right" | "middle",
        locationMode: s.locationMode,
        fixedX: clickX,
        fixedY: clickY,
        onlyTargetFocused: s.onlyTargetFocused,
      });
    } catch (err) {
      useEngineStore.getState().setStatus(`Failed: ${String(err)}`);
      log(`Error: ${String(err)}`);
    }
  },

  stop: async () => {
    try {
      await api.stopKeyClick();
    } catch (err) {
      useEngineStore.getState().setStatus(`Failed: ${String(err)}`);
    }
  },
}));
