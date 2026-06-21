import { create } from "zustand";

import type { Mode, TimelineView, ValidationErrorResponse } from "../lib/types";

/** Why the variable form is open: just set values, or set values then start a run. */
export type VariableDialogMode = "edit" | "run";

interface UiStore {
  mode: Mode;
  timelineView: TimelineView;
  selectedActionIdx: number | null;
  showVariableDialog: boolean;
  variableDialogMode: VariableDialogMode;
  yamlContent: string;
  yamlErrors: ValidationErrorResponse[];

  setMode: (mode: Mode) => void;
  setTimelineView: (view: TimelineView) => void;
  selectAction: (index: number | null) => void;
  setShowVariableDialog: (show: boolean) => void;
  openVariableDialog: (mode: VariableDialogMode) => void;
  setYamlContent: (content: string) => void;
  setYamlErrors: (errors: ValidationErrorResponse[]) => void;
}

export const useUiStore = create<UiStore>((set) => ({
  mode: "simple",
  timelineView: "list",
  selectedActionIdx: null,
  showVariableDialog: false,
  variableDialogMode: "edit",
  yamlContent: "",
  yamlErrors: [],

  setMode: (mode) => set({ mode }),
  setTimelineView: (timelineView) => set({ timelineView }),
  selectAction: (selectedActionIdx) => set({ selectedActionIdx }),
  setShowVariableDialog: (showVariableDialog) => set({ showVariableDialog }),
  openVariableDialog: (variableDialogMode) => set({ variableDialogMode, showVariableDialog: true }),
  setYamlContent: (yamlContent) => set({ yamlContent }),
  setYamlErrors: (yamlErrors) => set({ yamlErrors }),
}));
