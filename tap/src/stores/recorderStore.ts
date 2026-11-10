import { create } from "zustand";

import { api } from "../lib/ipc";
import type { RecorderState, RecordingStatus } from "../lib/types";
import { useDocumentStore } from "./documentStore";
import { useUiStore } from "./uiStore";
import { log } from "./logStore";

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

export const useRecorderStore = create<RecorderStore>((set) => ({
  recorderState: "Idle",
  eventCount: 0,
  durationMs: 0,

  setStatus: (status) =>
    set({ recorderState: status.state, eventCount: status.event_count, durationMs: status.duration_ms }),

  start: async () => {
    try {
      // Whether positions are recorded relative to the target window is decided by the backend's
      // document: make sure it has seen the latest edits (the toggle included).
      await useDocumentStore.getState().flush();
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
      const doc = await api.stopRecording();
      set({ recorderState: "Idle" });
      useDocumentStore.getState().applyDocument(doc);
      useUiStore.getState().setMode("timeline");
      log(`Recording stopped: ${doc.timeline.length} actions`);
    } catch (err) {
      log(`Failed to stop recording: ${String(err)}`);
    }
  },
}));
