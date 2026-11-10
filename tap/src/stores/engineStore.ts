import { create } from "zustand";

import { formatResolvedAction } from "../lib/actions";
import { api } from "../lib/ipc";
import type { EngineEvent, EngineState } from "../lib/types";
import { clearLog, log } from "./logStore";

interface EngineStore {
  engineState: EngineState;
  countdown: number | null;
  executedCount: number;
  iteration: number;
  lastAction: string | null;
  status: string;
  targetWindowMatched: boolean;

  setStatus: (status: string) => void;
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
  targetWindowMatched: true,

  setStatus: (status) => set({ status }),

  resetRunStats: () => {
    set({ executedCount: 0, iteration: 0 });
    clearLog();
  },

  handleEngineEvent: (e) => {
    if (typeof e === "string") {
      if (e === "Completed") {
        set({ status: "Completed" });
        log("All done");
      } else if (e === "TargetWindowFocused") {
        set({ targetWindowMatched: true, status: "Running" });
        log("Target window focused");
      } else {
        log(`(unknown engine event) ${e}`);
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
      log(`State: ${e.StateChanged.old} -> ${next}`);
    } else if ("CountdownTick" in e) {
      set({ countdown: e.CountdownTick.remaining_secs, status: `Starting in ${e.CountdownTick.remaining_secs}...` });
    } else if ("ActionStarting" in e) {
      const actionStr = formatResolvedAction(e.ActionStarting.action);
      set({ lastAction: actionStr, status: `Executing: ${actionStr}` });
      log(`-> ${actionStr}`);
    } else if ("ActionCompleted" in e) {
      set((s) => ({ executedCount: s.executedCount + 1 }));
    } else if ("IterationCompleted" in e) {
      set({ iteration: e.IterationCompleted.iteration });
      log(`Iteration #${e.IterationCompleted.iteration}`);
    } else if ("Error" in e) {
      set({ status: `Error: ${e.Error.message}` });
      log(`Error: ${e.Error.message}`);
    } else if ("WaitingForCondition" in e) {
      set({ status: `Waiting: ${e.WaitingForCondition.condition}` });
      log(`Waiting: ${e.WaitingForCondition.condition}`);
    } else if ("ConditionSatisfied" in e) {
      log(`Condition met: ${e.ConditionSatisfied.condition}`);
    } else if ("ConditionTimeout" in e) {
      log(`Timeout: ${e.ConditionTimeout.condition}`);
    } else if ("CounterChanged" in e) {
      log(`${e.CounterChanged.key} = ${e.CounterChanged.value}`);
    } else if ("TargetWindowUnfocused" in e) {
      set({ targetWindowMatched: false, status: "Target window not focused" });
      log("Target window lost focus");
    } else {
      log(`(unknown engine event) ${JSON.stringify(e)}`);
    }
  },

  onEmergencyStop: () => {
    set({ status: "Emergency stopped!" });
    log("EMERGENCY STOP");
  },

  pause: () => guard(get, () => api.pauseExecution(), "Paused"),
  resume: () => guard(get, () => api.resumeExecution(), "Resumed"),
  stop: () => guard(get, () => api.stopExecution(), "Stopped"),
  emergencyStop: () => guard(get, () => api.emergencyStop(), "Emergency stop requested"),
}));

async function guard(get: () => EngineStore, fn: () => Promise<void>, okLog: string): Promise<void> {
  try {
    await fn();
    log(okLog);
  } catch (err) {
    get().setStatus(`Failed: ${String(err)}`);
    log(`Error: ${String(err)}`);
  }
}
