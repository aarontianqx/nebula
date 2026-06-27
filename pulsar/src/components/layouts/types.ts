import type { ParamValue, ToolDescriptor } from "../../types/tool";

/** 各 archetype 布局共享的 props（来自 toolStore）。 */
export interface LayoutProps {
  tool: ToolDescriptor;
  input: string;
  params: Record<string, ParamValue>;
  output: string;
  error: string | null;
  running: boolean;
  autoRun: boolean;
  setInput: (v: string) => void;
  setParam: (key: string, value: ParamValue) => void;
  run: () => void;
}
