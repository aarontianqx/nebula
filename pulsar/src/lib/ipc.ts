// IPC 边界：所有后端调用都经过这里。
// 带 guard——在没有 Tauri 运行时（纯浏览器预览）时仍可渲染，仅调用时报错。

import type { DetectionResult, ParamValue, ToolDescriptor } from "../types/tool";

function hasTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!hasTauri()) {
    throw new Error("Tauri 运行时不可用（请用 `yarn tauri:dev` 启动）。");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export interface RunRequest {
  id: string;
  input: string;
  params: Record<string, ParamValue>;
}

export const api = {
  isAvailable: hasTauri,

  listTools(): Promise<ToolDescriptor[]> {
    return invoke<ToolDescriptor[]>("list_tools");
  },

  searchTools(query: string): Promise<ToolDescriptor[]> {
    return invoke<ToolDescriptor[]>("search_tools", { query });
  },

  runTool(request: RunRequest): Promise<string> {
    return invoke<string>("run_tool", { request });
  },

  detect(input: string): Promise<DetectionResult[]> {
    return invoke<DetectionResult[]>("detect", { input });
  },
};
