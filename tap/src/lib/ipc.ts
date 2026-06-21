// Thin, typed wrapper around the Tauri IPC surface.
//
// Every backend command is exposed here so stores/components never touch raw
// `invoke` strings. Calls are guarded so the UI still renders in a plain
// browser (e.g. `vite dev` smoke tests) where the Tauri runtime is absent.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ColorResponse,
  EngineState,
  Profile,
  RecordingStatus,
  Timeline,
  ValidationErrorResponse,
  VariableDefinitionResponse,
  WindowInfoResponse,
} from "./types";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error(`Tauri runtime unavailable; "${cmd}" cannot run in a plain browser.`);
  }
  return tauriInvoke<T>(cmd, args);
}

/** Subscribe to a backend event; returns a no-op unsubscribe outside Tauri. */
export async function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => {};
  }
  return tauriListen<T>(event, handler);
}

export const api = {
  // === Engine lifecycle ===
  getState: () => invoke<EngineState>("get_state"),
  startExecution: () => invoke<void>("start_execution"),
  pauseExecution: () => invoke<void>("pause_execution"),
  resumeExecution: () => invoke<void>("resume_execution"),
  stopExecution: () => invoke<void>("stop_execution"),
  emergencyStop: () => invoke<void>("emergency_stop"),

  // === Document / profile (sync contract) ===
  updateProfile: (profile: Profile) => invoke<void>("update_profile", { profile }),
  getCurrentProfile: () => invoke<Profile>("get_current_profile"),
  saveProfile: (name: string) => invoke<string>("cmd_save_profile", { name }),
  loadProfile: (name: string) => invoke<Profile>("cmd_load_profile", { name }),
  deleteProfile: (name: string) => invoke<void>("cmd_delete_profile", { name }),
  listProfiles: () => invoke<string[]>("cmd_list_profiles"),
  getLastUsed: () => invoke<string | null>("cmd_get_last_used"),

  setSimpleRepeat: (args: {
    actionType: string;
    x: number | null;
    y: number | null;
    key: string | null;
    intervalMs: number;
    repeatCount: number | null;
    countdownSecs: number;
  }) => invoke<void>("set_simple_repeat", args),

  // === Recording ===
  startRecording: () => invoke<void>("start_recording"),
  pauseRecording: () => invoke<void>("pause_recording"),
  resumeRecording: () => invoke<void>("resume_recording"),
  stopRecording: () => invoke<Timeline>("stop_recording"),
  getRecordingStatus: () => invoke<RecordingStatus>("get_recording_status"),

  // === Windows / pixels ===
  listWindows: () => invoke<WindowInfoResponse[]>("cmd_list_windows"),
  getForegroundWindow: () => invoke<WindowInfoResponse | null>("cmd_get_foreground_window"),
  getPixelColor: (x: number, y: number) => invoke<ColorResponse | null>("cmd_get_pixel_color", { x, y }),

  // === Picker ===
  openPicker: () => invoke<void>("open_picker_window"),

  // === YAML / variables ===
  exportYaml: () => invoke<string>("cmd_export_yaml"),
  importYaml: (yamlContent: string) => invoke<Profile>("cmd_import_yaml", { yamlContent }),
  validateYaml: (yamlContent: string) =>
    invoke<ValidationErrorResponse[] | null>("cmd_validate_yaml", { yamlContent }),
  getMacroVariables: () => invoke<VariableDefinitionResponse[]>("cmd_get_macro_variables"),
  setRuntimeVariables: (vars: Record<string, unknown>) =>
    invoke<void>("cmd_set_runtime_variables", { vars }),

  // === Key-to-Click ===
  startKeyClick: (intervalMs: number, holdDelayMs: number) =>
    invoke<void>("start_key_click", { intervalMs, holdDelayMs }),
  stopKeyClick: () => invoke<void>("stop_key_click"),
};
