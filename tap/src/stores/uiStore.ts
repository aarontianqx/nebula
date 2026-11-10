import { create } from "zustand";

import { api } from "../lib/ipc";
import type { Mode, TimelineView, ValidationErrorResponse } from "../lib/types";
import { log } from "./logStore";

/** Why the variable form is open: just set values, or set values then start a run. */
export type VariableDialogMode = "edit" | "run";

interface UiStore {
  mode: Mode;
  timelineView: TimelineView;
  selectedActionIdx: number | null;
  showVariableDialog: boolean;
  variableDialogMode: VariableDialogMode;
  /** The editor for the macro's variable definitions (name, type, default, description). */
  showVariableDefinitions: boolean;
  yamlContent: string;
  yamlErrors: ValidationErrorResponse[];
  /** When true, the backend skips real injection (safe preview of a run). */
  dryRun: boolean;

  setMode: (mode: Mode) => void;
  setTimelineView: (view: TimelineView) => void;
  selectAction: (index: number | null) => void;
  setShowVariableDialog: (show: boolean) => void;
  openVariableDialog: (mode: VariableDialogMode) => void;
  setShowVariableDefinitions: (show: boolean) => void;
  setYamlContent: (content: string) => void;
  setYamlErrors: (errors: ValidationErrorResponse[]) => void;
  setDryRun: (enabled: boolean) => void;
  loadDryRun: () => Promise<void>;
}

export const useUiStore = create<UiStore>((set) => ({
  mode: "simple",
  timelineView: "list",
  selectedActionIdx: null,
  showVariableDialog: false,
  variableDialogMode: "edit",
  showVariableDefinitions: false,
  yamlContent: "",
  yamlErrors: [],
  dryRun: false,

  setMode: (mode) => set({ mode }),
  setTimelineView: (timelineView) => set({ timelineView }),
  selectAction: (selectedActionIdx) => set({ selectedActionIdx }),
  setShowVariableDialog: (showVariableDialog) => set({ showVariableDialog }),
  openVariableDialog: (variableDialogMode) => set({ variableDialogMode, showVariableDialog: true }),
  setShowVariableDefinitions: (showVariableDefinitions) => set({ showVariableDefinitions }),
  setYamlContent: (yamlContent) => set({ yamlContent }),
  setYamlErrors: (yamlErrors) => set({ yamlErrors }),

  setDryRun: (dryRun) => {
    set({ dryRun });
    void api
      .setDryRun(dryRun)
      .then(() => log(dryRun ? "Dry-run ON (no real input)" : "Dry-run OFF"))
      .catch((err) => log(`Failed to set dry-run: ${String(err)}`));
  },

  loadDryRun: async () => {
    try {
      set({ dryRun: await api.getDryRun() });
    } catch {
      // Backend unavailable (plain browser); keep the default.
    }
  },
}));
