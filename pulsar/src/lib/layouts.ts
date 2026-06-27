// 工具的「呈现原型」分类。
//
// 现状：所有工具都用「大文本框 → 大文本框」，但 30 个工具里只有约一半真正适合。
// 这里按工具的输入/输出形态把它们归到 5 种 archetype，前端据此选择最贴合的布局与组件。
//
// 暂以 tool id 映射（前端单一来源），设计稳定后可上提到 Rust descriptor。

export type Archetype =
  | "transform" // 双栏：大文本输入 → 大文本输出（编码/格式化/批量文本）
  | "inspect" // 紧凑输入 → 结构化字段卡（时间戳/进制/颜色/JWT/哈希/Cron）
  | "generate" // 无输入：纯参数表单 → 结果（密码/ID）
  | "query" // 查询字段 + 主体文本 → 匹配/结果（正则/JSONPath/Diff）
  | "visual"; // 渲染输出（二维码）

/** 显式分类（未列出的工具默认 transform）。 */
const ARCHETYPE_BY_ID: Record<string, Archetype> = {
  // inspect：短输入或单段输入，输出是若干结构化字段
  "converters.timestamp": "inspect",
  "converters.number_base": "inspect",
  "converters.color": "inspect",
  "converters.cron": "inspect",
  "encoders.jwt": "inspect",
  "generators.hash": "inspect",
  "text.stats": "inspect",

  // generate：没有有意义的输入，主体是参数表单
  "generators.password": "generate",
  "generators.id": "generate",

  // query：需要一个独立「查询」字段 + 主体
  "testers.regex": "query",
  "testers.jsonpath": "query",
  "testers.diff": "query",

  // visual：输出需要渲染
  "generators.qr": "visual",
};

export function archetypeOf(toolId: string): Archetype {
  return ARCHETYPE_BY_ID[toolId] ?? "transform";
}

/** inspect 工具的输入是否为单行（短输入框而非多行文本域）。 */
const SINGLE_LINE_INPUT = new Set<string>([
  "converters.timestamp",
  "converters.number_base",
  "converters.color",
  "converters.cron",
  "encoders.jwt",
]);

export function isSingleLineInput(toolId: string): boolean {
  return SINGLE_LINE_INPUT.has(toolId);
}

/** 输入占位提示（更具体、可上手）。inspect 与 transform 都适用。 */
const INPUT_HINTS: Record<string, string> = {
  // inspect
  "converters.timestamp": "如 1700000000 或留空取当前时间",
  "converters.number_base": "如 255 / 0xff / 0b1010 / 0o17",
  "converters.color": "如 #5b8cff / rgb(91,140,255) / hsl(225,100%,68%)",
  "converters.cron": "如 */5 * * * *",
  "encoders.jwt": "粘贴 JWT（header.payload.signature）",
  "generators.hash": "要计算哈希的文本",
  "text.stats": "要统计的文本",

  // transform — encoders / converters / formatters / text
  "encoders.base64": "要编码或解码的文本",
  "encoders.url": "要编码或解码的 URL / 文本",
  "encoders.hex": "要编码或解码的文本",
  "encoders.unicode": "要转义或还原的文本",
  "encoders.html_entity": "要转义或还原的 HTML 文本",
  "converters.json_yaml": "粘贴 JSON（或切到 YAML→JSON）",
  "converters.json_csv": "粘贴 JSON 对象数组（或切到 CSV→JSON）",
  "converters.xml_json": "粘贴 XML（或切到 JSON→XML）",
  "converters.toml": "粘贴 TOML（或切换其它方向）",
  "formatters.json": "粘贴要格式化或压缩的 JSON",
  "formatters.sql": "粘贴要格式化的 SQL",
  "formatters.xml": "粘贴要格式化的 XML",
  "text.case": "要转换命名风格的标识符 / 短语",
  "text.dedup_sort": "每行一条，按行去重 / 排序",
  "text.slug": "要生成 slug 的标题 / 短语",
};

export function inputHint(toolId: string): string | undefined {
  return INPUT_HINTS[toolId];
}

/**
 * 一键填充的示例输入（用于「示例」按钮）。
 *
 * 约定：示例必须在工具的**默认参数**下产出有意义的结果，
 * 这样用户点一下示例 + 默认设置就能立刻看到效果。
 */
const EXAMPLES: Record<string, string> = {
  // inspect
  "converters.timestamp": "1700000000",
  "converters.number_base": "255",
  "converters.color": "#5b8cff",
  "converters.cron": "*/15 9-18 * * 1-5",
  "encoders.jwt":
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
  "generators.hash": "hello world",
  "text.stats": "The quick brown fox\njumps over the lazy dog.",

  // transform — encoders（默认 encode）
  "encoders.base64": "hello world",
  "encoders.url": "https://example.com/search?q=hello world&lang=中文",
  "encoders.hex": "Pulsar ⚡",
  "encoders.unicode": "Pulsar 脉冲星 ⚡",
  "encoders.html_entity": '<div class="box">5 < 10 && "ok"</div>',

  // transform — converters（默认方向见各工具）
  "converters.json_yaml": '{"name":"pulsar","tags":["dev","local"],"stars":42}',
  "converters.json_csv":
    '[{"id":1,"name":"Ann","city":"NYC"},{"id":2,"name":"Bo","city":"LA"}]',
  "converters.xml_json":
    '<user id="1"><name>Ann</name><roles><role>admin</role><role>dev</role></roles></user>',
  "converters.toml": 'title = "Pulsar"\n\n[owner]\nname = "Ann"\nstars = 42',

  // transform — formatters
  "formatters.json": '{"name":"pulsar","tags":["dev","local"],"stars":42}',
  "formatters.sql":
    "select id,name,email from users where age>18 and city='NYC' order by name desc",
  "formatters.xml":
    '<note><to>Ann</to><from>Bo</from><body>hello</body></note>',

  // transform — text
  "text.case": "hello world example",
  "text.dedup_sort": "banana\napple\ncherry\napple\nBanana\n\ncherry",
  "text.slug": "Hello World! This is Pulsar 🚀",
};

export function exampleFor(toolId: string): string | undefined {
  return EXAMPLES[toolId];
}

/** 轻量工具：输入即时出结果（防抖自动运行），无需手点运行。 */
const AUTO_RUN = new Set<string>([
  "converters.timestamp",
  "converters.number_base",
  "converters.color",
  "encoders.jwt",
  "encoders.base64",
  "encoders.hex",
  "encoders.url",
  "encoders.unicode",
  "generators.hash",
  "text.stats",
  "text.case",
  "text.slug",
  "formatters.json",
  "generators.qr",
]);

export function isAutoRun(toolId: string): boolean {
  return AUTO_RUN.has(toolId);
}

/**
 * 即使输入为空也应运行的工具（输出依赖「当前时间」等隐式状态）。
 * 这类工具选中时立即出结果，无需用户先输入。
 */
const RUNS_ON_EMPTY = new Set<string>(["converters.timestamp"]);

export function runsOnEmpty(toolId: string): boolean {
  return RUNS_ON_EMPTY.has(toolId);
}

// ── 输出解析（inspect 字段卡）────────────────────────────────

export interface OutputField {
  label: string;
  value: string;
}

export interface ParsedSection {
  title: string;
  body: string;
}

/**
 * 把 `LABEL: value` 形式的输出解析成字段数组。
 * 仅当绝大多数非空行都符合该形态时才返回，否则返回 null（交给纯文本渲染）。
 */
export function parseFields(output: string): OutputField[] | null {
  const lines = output.split("\n").filter((l) => l.trim().length > 0);
  if (lines.length === 0) return null;

  const fields: OutputField[] = [];
  let matched = 0;
  for (const line of lines) {
    // label 允许中英文/空格/括号/斜杠/连字符，冒号后至少一个空格
    const m = line.match(/^([\w \u4e00-\u9fa5()/-]+?):\s+(.+)$/);
    if (m) {
      matched += 1;
      fields.push({ label: m[1].trim(), value: m[2].trim() });
    } else {
      fields.push({ label: "", value: line.trim() });
    }
  }
  // 至少 60% 的行是 key:value 才认为是字段型输出
  return matched / lines.length >= 0.6 ? fields : null;
}

/**
 * 拆分 generate 类输出为「主结果 + 附加信息」。
 *
 * 约定：首个空行之前是主结果（要复制的值，可多行，如批量 ID），
 * 之后的 `LABEL: value` 行为元信息（如密码的长度/熵）。
 * 复制时只复制主结果，避免把元信息也粘出去。
 */
export function splitGenerateResult(output: string): {
  primary: string;
  meta: OutputField[];
} {
  const idx = output.indexOf("\n\n");
  if (idx === -1) return { primary: output.trim(), meta: [] };
  const primary = output.slice(0, idx).trim();
  const rest = output.slice(idx + 2);
  const meta = parseFields(rest) ?? [];
  // 只保留确有 label 的元信息行。
  return { primary, meta: meta.filter((f) => f.label) };
}

/**
 * 解析 `--- Title ---\n body` 形式的分段输出（如 JWT 的 Header / Payload）。
 */
export function parseSections(output: string): ParsedSection[] | null {
  if (!output.includes("---")) return null;
  const re = /---\s*(.+?)\s*---\n([\s\S]*?)(?=\n---|\s*$)/g;
  const sections: ParsedSection[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(output)) !== null) {
    sections.push({ title: m[1].trim(), body: m[2].trim() });
  }
  return sections.length > 0 ? sections : null;
}
