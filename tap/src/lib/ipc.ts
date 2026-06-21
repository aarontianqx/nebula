// Thin, typed wrapper around the Tauri IPC surface.
//
// Every backend command is exposed here so stores/components never touch raw
// `invoke` strings. Calls are guarded so the UI still renders in a plain
// browser (e.g. `vite dev` smoke tests) where the Tauri runtime is absent.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ColorResponse,
  DocumentMeta,
  EngineState,
  KeyClickRequest,
  PermissionStatus,
  Profile,
  RecordingStatus,
  TemplateInfo,
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

const YAML_FILTER = [{ name: "Macro (YAML)", extensions: ["yaml", "yml"] }];

/** Native "save file" dialog; returns the chosen path, or null if cancelled/unavailable. */
export async function pickSavePath(defaultName: string): Promise<string | null> {
  if (!isTauri()) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  const path = await save({ defaultPath: defaultName, filters: YAML_FILTER });
  return path ?? null;
}

/** Native "open file" dialog; returns the chosen path, or null if cancelled/unavailable. */
export async function pickOpenPath(): Promise<string | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({ multiple: false, directory: false, filters: YAML_FILTER });
  return typeof result === "string" ? result : null;
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
  getRecentProfiles: () => invoke<string[]>("cmd_get_recent_profiles"),

  // === Metadata (lossless) ===
  getDocumentMeta: () => invoke<DocumentMeta>("cmd_get_document_meta"),
  setDocumentMeta: (meta: DocumentMeta) =>
    invoke<void>("cmd_set_document_meta", {
      description: meta.description,
      author: meta.author,
      tags: meta.tags,
    }),

  // === Templates ===
  listTemplates: () => invoke<TemplateInfo[]>("cmd_list_templates"),
  applyTemplate: (id: string) => invoke<Profile>("cmd_apply_template", { id }),

  // === Native file import / export ===
  exportYamlToPath: (path: string) => invoke<void>("cmd_export_yaml_to_path", { path }),
  importYamlFromPath: (path: string) => invoke<Profile>("cmd_import_yaml_from_path", { path }),

  // === OS permissions ===
  checkPermissions: () => invoke<PermissionStatus>("cmd_check_permissions"),
  requestScreenRecording: () => invoke<PermissionStatus>("cmd_request_screen_recording"),
  openPermissionSettings: (which: "accessibility" | "screen_recording") =>
    invoke<void>("cmd_open_permission_settings", { which }),

  setSimpleRepeat: (args: {
    actionType: string;
    x: number | null;
    y: number | null;
    key: string | null;
    button: string | null;
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
  startKeyClick: (request: KeyClickRequest) => invoke<void>("start_key_click", { request }),
  stopKeyClick: () => invoke<void>("stop_key_click"),

  // === Dry-run + key capture ===
  setDryRun: (enabled: boolean) => invoke<void>("cmd_set_dry_run", { enabled }),
  getDryRun: () => invoke<boolean>("cmd_get_dry_run"),
  captureKey: () => invoke<string>("cmd_capture_key"),
};
