import { create } from "zustand";

import { formatAction } from "../lib/actions";
import { api } from "../lib/ipc";
import type { EngineEvent, EngineState, LogEntry, Point } from "../lib/types";

const LOG_CAP = 200;

interface EngineStore {
  engineState: EngineState;
  countdown: number | null;
  executedCount: number;
  iteration: number;
  lastAction: string | null;
  status: string;
  uiMessage: string | null;
  targetWindowMatched: boolean;
  mousePos: Point | null;
  logs: LogEntry[];

  addLog: (message: string) => void;
  setStatus: (status: string) => void;
  setUiMessage: (message: string | null) => void;
  setMousePos: (pos: Point) => void;
  resetRunStats: () => void;
  handleEngineEvent: (event: EngineEvent) => void;
  onEmergencyStop: () => void;

  pause: () => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  emergencyStop: () => Promise<void>;
}

export const useEngineStore = create<EngineStore>((set, get) => ({
  engineState: "Idle",
  countdown: null,
  executedCount: 0,
  iteration: 0,
  lastAction: null,
  status: "Ready",
  uiMessage: null,
  targetWindowMatched: true,
  mousePos: null,
  logs: [],

  addLog: (message) =>
    set((s) => ({
      logs: [...s.logs, { time: nowTime(), message }].slice(-LOG_CAP),
    })),

  setStatus: (status) => set({ status }),
  setUiMessage: (uiMessage) => set({ uiMessage }),
  setMousePos: (mousePos) => set({ mousePos }),

  resetRunStats: () => set({ executedCount: 0, iteration: 0, logs: [] }),

  handleEngineEvent: (e) => {
    const { addLog } = get();

    if (typeof e === "string") {
      if (e === "Completed") {
        set({ status: "Completed" });
        addLog("All done");
      } else if (e === "TargetWindowFocused") {
        set({ targetWindowMatched: true, status: "Running" });
        addLog("Target window focused");
      } else {
        addLog(`(unknown engine event) ${e}`);
      }
      return;
    }

    if ("StateChanged" in e) {
      const next = e.StateChanged.new;
      const patch: Partial<EngineStore> = { engineState: next };
      if (next === "Idle") {
        patch.countdown = null;
        patch.lastAction = null;
        patch.status = "Ready";
      } else if (next === "Running") {
        patch.status = "Running";
      } else if (next === "Paused") {
        patch.status = "Paused";
      } else if (next === "Arming") {
        patch.status = "Arming...";
      }
      set(patch);
      addLog(`State: ${e.StateChanged.old} -> ${next}`);
    } else if ("CountdownTick" in e) {
      set({ countdown: e.CountdownTick.remaining_secs, status: `Starting in ${e.CountdownTick.remaining_secs}...` });
    } else if ("ActionStarting" in e) {
      const actionStr = formatAction(e.ActionStarting.action);
      set({ lastAction: actionStr, status: `Executing: ${actionStr}` });
      addLog(`-> ${actionStr}`);
    } else if ("ActionCompleted" in e) {
      set((s) => ({ executedCount: s.executedCount + 1 }));
    } else if ("IterationCompleted" in e) {
      set({ iteration: e.IterationCompleted.iteration });
      addLog(`Iteration #${e.IterationCompleted.iteration}`);
    } else if ("Error" in e) {
      set({ status: `Error: ${e.Error.message}` });
      addLog(`Error: ${e.Error.message}`);
    } else if ("WaitingForCondition" in e) {
      set({ status: `Waiting: ${e.WaitingForCondition.condition}` });
      addLog(`Waiting: ${e.WaitingForCondition.condition}`);
    } else if ("ConditionSatisfied" in e) {
      addLog(`Condition met: ${e.ConditionSatisfied.condition}`);
    } else if ("ConditionTimeout" in e) {
      addLog(`Timeout: ${e.ConditionTimeout.condition}`);
    } else if ("CounterChanged" in e) {
      addLog(`${e.CounterChanged.key} = ${e.CounterChanged.value}`);
    } else if ("TargetWindowUnfocused" in e) {
      set({ targetWindowMatched: false, status: "Target window not focused" });
      addLog("Target window lost focus");
    } else {
      addLog(`(unknown engine event) ${JSON.stringify(e)}`);
    }
  },

  onEmergencyStop: () => {
    set({ status: "Emergency stopped!" });
    get().addLog("EMERGENCY STOP");
  },

  pause: () => guard(get, () => api.pauseExecution(), "Paused"),
  resume: () => guard(get, () => api.resumeExecution(), "Resumed"),
  stop: () => guard(get, () => api.stopExecution(), "Stopped"),
  emergencyStop: () => guard(get, () => api.emergencyStop(), "Emergency stop requested"),
}));

function nowTime(): string {
  const now = new Date();
  const h = now.getHours().toString().padStart(2, "0");
  const m = now.getMinutes().toString().padStart(2, "0");
  const s = now.getSeconds().toString().padStart(2, "0");
  const ms = now.getMilliseconds().toString().padStart(3, "0");
  return `${h}:${m}:${s}.${ms}`;
}

async function guard(get: () => EngineStore, fn: () => Promise<void>, okLog: string): Promise<void> {
  try {
    await fn();
    get().addLog(okLog);
  } catch (err) {
    get().setStatus(`Failed: ${String(err)}`);
    get().addLog(`Error: ${String(err)}`);
  }
}
