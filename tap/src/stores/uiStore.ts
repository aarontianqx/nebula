import { create } from "zustand";

import type { Mode, TimelineView, ValidationErrorResponse } from "../lib/types";

interface UiStore {
  mode: Mode;
  timelineView: TimelineView;
  selectedActionIdx: number | null;
  showVariableDialog: boolean;
  yamlContent: string;
  yamlErrors: ValidationErrorResponse[];

  setMode: (mode: Mode) => void;
  setTimelineView: (view: TimelineView) => void;
  selectAction: (index: number | null) => void;
  setShowVariableDialog: (show: boolean) => void;
  setYamlContent: (content: string) => void;
  setYamlErrors: (errors: ValidationErrorResponse[]) => void;
}

export const useUiStore = create<UiStore>((set) => ({
  mode: "simple",
  timelineView: "list",
  selectedActionIdx: null,
  showVariableDialog: false,
  yamlContent: "",
  yamlErrors: [],

  setMode: (mode) => set({ mode }),
  setTimelineView: (timelineView) => set({ timelineView }),
  selectAction: (selectedActionIdx) => set({ selectedActionIdx }),
  setShowVariableDialog: (showVariableDialog) => set({ showVariableDialog }),
  setYamlContent: (yamlContent) => set({ yamlContent }),
  setYamlErrors: (yamlErrors) => set({ yamlErrors }),
}));
