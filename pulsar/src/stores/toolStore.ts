// 当前选中工具的状态：输入、参数、输出、运行。

import { create } from "zustand";
import { api } from "../lib/ipc";
import type { ParamSpec, ParamValue, ToolDescriptor } from "../types/tool";

interface ToolState {
  active: ToolDescriptor | null;
  input: string;
  params: Record<string, ParamValue>;
  output: string;
  error: string | null;
  running: boolean;

  selectTool: (tool: ToolDescriptor) => void;
  selectAndFill: (tool: ToolDescriptor, input: string) => void;
  setInput: (input: string) => void;
  setParam: (key: string, value: ParamValue) => void;
  run: () => Promise<void>;
}

/** 按 ParamSpec 把 default 字符串解释为对应类型的初值。 */
function defaultParams(specs: ParamSpec[]): Record<string, ParamValue> {
  const out: Record<string, ParamValue> = {};
  for (const s of specs) {
    if (s.kind === "bool") out[s.key] = s.default === "true";
    else if (s.kind === "int") out[s.key] = Number(s.default) || 0;
    else out[s.key] = s.default;
  }
  return out;
}

export const useToolStore = create<ToolState>((set, get) => ({
  active: null,
  input: "",
  params: {},
  output: "",
  error: null,
  running: false,

  selectTool: (tool) =>
    set({
      active: tool,
      params: defaultParams(tool.params),
      output: "",
      error: null,
    }),

  /** 由 Smart Detection 跳转：选中工具并预填输入，清空旧输出。 */
  selectAndFill: (tool, input) =>
    set({
      active: tool,
      params: defaultParams(tool.params),
      input,
      output: "",
      error: null,
    }),

  setInput: (input) => set({ input }),

  setParam: (key, value) =>
    set((state) => ({ params: { ...state.params, [key]: value } })),

  run: async () => {
    const { active, input, params } = get();
    if (!active) return;
    set({ running: true, error: null });
    try {
      const output = await api.runTool({ id: active.id, input, params });
      set({ output, running: false });
    } catch (e) {
      set({ error: String(e), output: "", running: false });
    }
  },
}));
