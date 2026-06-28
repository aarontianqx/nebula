// 当前选中工具的状态：输入、参数、输出、运行。

import { create } from "zustand";
import { api } from "../lib/ipc";
import type { ParamSpec, ParamValue, ToolDescriptor } from "../types/tool";

/** 单个工具的会话快照（内存级，切页时暂存与恢复）。 */
interface ToolSession {
  input: string;
  params: Record<string, ParamValue>;
  output: string;
  error: string | null;
}

interface ToolState {
  active: ToolDescriptor | null;
  input: string;
  params: Record<string, ParamValue>;
  output: string;
  error: string | null;
  running: boolean;
  /**
   * 按工具 id 暂存的会话（输入/参数/输出/错误）。仅存在于内存：
   * 切换工具时存当前、切回时恢复，刷新/重启即丢失（符合预期，不持久化）。
   */
  sessions: Record<string, ToolSession>;

  selectTool: (tool: ToolDescriptor) => void;
  selectAndFill: (tool: ToolDescriptor, input: string) => void;
  setInput: (input: string) => void;
  setParam: (key: string, value: ParamValue) => void;
  run: () => Promise<void>;
  clearOutput: () => void;
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

/** 把当前 active 工具的状态写回 sessions（切走前调用）。 */
function snapshot(state: ToolState): Record<string, ToolSession> {
  if (!state.active) return state.sessions;
  return {
    ...state.sessions,
    [state.active.id]: {
      input: state.input,
      params: { ...state.params },
      output: state.output,
      error: state.error,
    },
  };
}

export const useToolStore = create<ToolState>((set, get) => ({
  active: null,
  input: "",
  params: {},
  output: "",
  error: null,
  running: false,
  sessions: {},

  selectTool: (tool) =>
    set((state) => {
      // 切回同一个工具：什么都不动，保持现状。
      if (state.active?.id === tool.id) return {};
      const sessions = snapshot(state);
      const prev = sessions[tool.id];
      return {
        active: tool,
        sessions,
        // 有上次会话则恢复"离开时的样子"，否则用默认值。
        input: prev?.input ?? "",
        params: prev?.params ?? defaultParams(tool.params),
        output: prev?.output ?? "",
        error: prev?.error ?? null,
      };
    }),

  /** 由 Smart Detection 跳转：选中工具并预填输入（覆盖该工具旧会话），清空旧输出。 */
  selectAndFill: (tool, input) =>
    set((state) => ({
      active: tool,
      sessions: snapshot(state),
      params: defaultParams(tool.params),
      input,
      output: "",
      error: null,
    })),

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

  clearOutput: () => set({ output: "", error: null }),
}));
