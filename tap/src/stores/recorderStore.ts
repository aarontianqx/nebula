import { create } from "zustand";

import { api } from "../lib/ipc";
import type { RecorderState, RecordingStatus } from "../lib/types";
import { useDocumentStore } from "./documentStore";
import { useEngineStore } from "./engineStore";
import { useUiStore } from "./uiStore";

interface RecorderStore {
  recorderState: RecorderState;
  eventCount: number;
  durationMs: number;

  setStatus: (status: RecordingStatus) => void;
  start: () => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
}

function log(msg: string): void {
  useEngineStore.getState().addLog(msg);
}

export const useRecorderStore = create<RecorderStore>((set) => ({
  recorderState: "Idle",
  eventCount: 0,
  durationMs: 0,

  setStatus: (status) =>
    set({ recorderState: status.state, eventCount: status.event_count, durationMs: status.duration_ms }),

  start: async () => {
    try {
      await api.startRecording();
      set({ recorderState: "Recording", eventCount: 0, durationMs: 0 });
      log("Recording started");
    } catch (err) {
      log(`Failed to start recording: ${String(err)}`);
    }
  },

  pause: async () => {
    try {
      await api.pauseRecording();
      log("Recording paused");
    } catch (err) {
      log(`Failed to pause recording: ${String(err)}`);
    }
  },

  resume: async () => {
    try {
      await api.resumeRecording();
      log("Recording resumed");
    } catch (err) {
      log(`Failed to resume recording: ${String(err)}`);
    }
  },

  stop: async () => {
    try {
      const timeline = await api.stopRecording();
      set({ recorderState: "Idle" });
      // The backend already set its canonical document; mirror it so edits/replay
      // operate on the recorded macro.
      await useDocumentStore.getState().refreshFromBackend();
      useUiStore.getState().setMode("timeline");
      log(`Recording stopped: ${timeline.actions.length} actions`);
    } catch (err) {
      log(`Failed to stop recording: ${String(err)}`);
    }
  },
}));
