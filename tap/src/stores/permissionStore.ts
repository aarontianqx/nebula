import { create } from "zustand";

import { api, isTauri } from "../lib/ipc";
import type { PermissionStatus } from "../lib/types";
import { useEngineStore } from "./engineStore";

function log(msg: string): void {
  useEngineStore.getState().addLog(msg);
}

interface PermissionStore extends PermissionStatus {
  /** True once a real probe has run (false means we're showing optimistic defaults). */
  checked: boolean;

  refresh: () => Promise<void>;
  requestScreenRecording: () => Promise<void>;
  openSettings: (which: "accessibility" | "screen_recording") => Promise<void>;
}

export const usePermissionStore = create<PermissionStore>((set) => ({
  // Optimistic defaults: assume granted until a probe says otherwise, so the UI
  // never blocks in a plain browser or before the first check resolves.
  accessibility: true,
  screen_recording: true,
  os: "other",
  checked: false,

  refresh: async () => {
    if (!isTauri()) return;
    try {
      const status = await api.checkPermissions();
      set({ ...status, checked: true });
    } catch (err) {
      log(`Failed to check permissions: ${String(err)}`);
    }
  },

  requestScreenRecording: async () => {
    if (!isTauri()) return;
    try {
      const status = await api.requestScreenRecording();
      set({ ...status, checked: true });
    } catch (err) {
      log(`Failed to request screen recording: ${String(err)}`);
    }
  },

  openSettings: async (which) => {
    if (!isTauri()) return;
    try {
      await api.openPermissionSettings(which);
    } catch (err) {
      log(`Failed to open settings: ${String(err)}`);
    }
  },
}));

/** Input hook + injection (record/replay/key-click) requires macOS Accessibility. */
export function selectInputReady(s: PermissionStore): boolean {
  return s.os !== "macos" || s.accessibility;
}

/** Pixel/window reads (color picker, conditions, target window) need Screen Recording. */
export function selectCaptureReady(s: PermissionStore): boolean {
  return s.os !== "macos" || s.screen_recording;
}
