// 与 Rust 的 ToolDescriptor 一一对应（serde snake_case）。

export type Category =
  | "converters"
  | "encoders"
  | "formatters"
  | "generators"
  | "testers"
  | "text"
  | "graphic"
  | "reference";

export type ParamKind = "bool" | "int" | "str" | "enum";

export type IoKind = "text" | "bytes";

export interface ParamSpec {
  key: string;
  label: string;
  kind: ParamKind;
  default: string;
  options: string[];
}

export interface ToolDescriptor {
  id: string;
  category: Category;
  name: string;
  description: string;
  keywords: string[];
  params: ParamSpec[];
  input_kind: IoKind;
  output_kind: IoKind;
  pipeable: boolean;
}

/** 单个参数的运行时值。 */
export type ParamValue = boolean | number | string;

/** Smart Detection 候选结果（对应 Rust DetectionResult）。 */
export interface DetectionResult {
  tool_id: string;
  tool_name: string;
  confidence: number;
}

export const CATEGORY_LABELS: Record<Category, string> = {
  converters: "Converters",
  encoders: "Encoders / Decoders",
  formatters: "Formatters",
  generators: "Generators",
  testers: "Testers",
  text: "Text",
  graphic: "Graphic",
  reference: "Reference",
};
