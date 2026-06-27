import { useEffect, useMemo, useState } from "react";
import { Code2, Eye, WrapText } from "lucide-react";
import type { ToolDescriptor } from "../types/tool";
import {
  archetypeOf,
  parseFields,
  parseSections,
  type OutputField,
  type ParsedSection,
} from "../lib/layouts";
import { countLabel } from "../lib/text";
import { CopyButton } from "./ui/CopyButton";
import { ResultFields } from "./ui/ResultFields";

interface Props {
  tool: ToolDescriptor;
  output: string;
  error: string | null;
  /** 不显示「输出」标题与外层 padding（嵌入式使用时）。 */
  bare?: boolean;
  /** 占位提示（无输出且无错误时）。 */
  placeholder?: string;
}

type Rendered =
  | { kind: "svg"; svg: string }
  | { kind: "color"; hex: string; fields: OutputField[] }
  | { kind: "sections"; sections: ParsedSection[] }
  | { kind: "fields"; fields: OutputField[] }
  | { kind: "text" };

/** 根据工具与输出内容，决定最贴合的渲染方式。 */
function decideRender(tool: ToolDescriptor, output: string): Rendered {
  const trimmed = output.trimStart();
  if (trimmed.startsWith("<svg") || trimmed.startsWith("<?xml")) {
    return { kind: "svg", svg: output };
  }
  if (tool.id === "converters.color") {
    const hex = output.match(/#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})/)?.[0];
    const fields = parseFields(output);
    if (hex && fields) return { kind: "color", hex, fields };
  }
  const sections = parseSections(output);
  if (sections) return { kind: "sections", sections };

  // 仅对 inspect 类工具尝试字段卡，避免误伤普通文本输出
  if (archetypeOf(tool.id) === "inspect") {
    const fields = parseFields(output);
    if (fields) return { kind: "fields", fields };
  }
  return { kind: "text" };
}

const RICH_KINDS = new Set(["svg", "color", "sections", "fields"]);

export function OutputView({ tool, output, error, bare = false, placeholder }: Props) {
  const rendered = useMemo(
    () => (output && !error ? decideRender(tool, output) : { kind: "text" as const }),
    [tool, output, error],
  );
  const canToggle = !error && RICH_KINDS.has(rendered.kind);
  const [mode, setMode] = useState<"render" | "source">("render");
  const [wrap, setWrap] = useState(true);

  useEffect(() => {
    setMode("render");
  }, [tool.id, output]);

  const showRendered = canToggle && mode === "render";
  // 纯文本（非富渲染）时才提供「自动换行」开关与字数徽标。
  const showText = !error && Boolean(output) && !showRendered;

  const body = error ? (
    <div className="flex items-start gap-2 rounded-lg border border-[var(--color-danger)]/40 bg-[var(--color-danger-soft)] px-3 py-2.5">
      <pre className="font-mono whitespace-pre-wrap text-sm leading-relaxed text-[var(--color-danger)]">
        {error}
      </pre>
    </div>
  ) : !output ? (
    <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-faint)]">
      {placeholder ?? "结果会显示在这里"}
    </div>
  ) : showRendered ? (
    <RenderBody rendered={rendered} />
  ) : (
    <pre
      className={[
        "font-mono text-sm leading-relaxed text-[var(--color-text)]",
        wrap ? "whitespace-pre-wrap break-words" : "whitespace-pre",
      ].join(" ")}
    >
      {output}
    </pre>
  );

  if (bare) return <div className="animate-fade-in h-full">{body}</div>;

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[var(--color-bg)]">
      <div className="flex items-center justify-between px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
            输出
          </span>
          {canToggle && <ViewToggle mode={mode} onChange={setMode} />}
        </div>
        <div className="flex items-center gap-2">
          {showText && output && (
            <span className="text-xs tabular-nums text-[var(--color-text-faint)]">
              {countLabel(output)}
            </span>
          )}
          {showText && <WrapToggle on={wrap} onToggle={() => setWrap((w) => !w)} />}
          <CopyButton text={output} />
        </div>
      </div>
      <div className="animate-fade-in flex-1 overflow-auto px-4 pb-4">{body}</div>
    </div>
  );
}

function WrapToggle({ on, onToggle }: { on: boolean; onToggle: () => void }) {
  return (
    <button
      onClick={onToggle}
      title={on ? "关闭自动换行" : "开启自动换行"}
      aria-pressed={on}
      className={[
        "flex items-center rounded p-1 transition-colors",
        on
          ? "text-[var(--color-accent)]"
          : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]",
      ].join(" ")}
    >
      <WrapText size={14} />
    </button>
  );
}

function ViewToggle({
  mode,
  onChange,
}: {
  mode: "render" | "source";
  onChange: (m: "render" | "source") => void;
}) {
  return (
    <div className="flex items-center rounded border border-[var(--color-border)] text-xs">
      <button
        onClick={() => onChange("render")}
        className={[
          "flex items-center gap-1 rounded-l px-2 py-0.5 transition-colors",
          mode === "render"
            ? "bg-[var(--color-accent)] text-white"
            : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]",
        ].join(" ")}
      >
        <Eye size={12} />
        渲染
      </button>
      <button
        onClick={() => onChange("source")}
        className={[
          "flex items-center gap-1 rounded-r px-2 py-0.5 transition-colors",
          mode === "source"
            ? "bg-[var(--color-accent)] text-white"
            : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]",
        ].join(" ")}
      >
        <Code2 size={12} />
        源码
      </button>
    </div>
  );
}

function RenderBody({ rendered }: { rendered: Rendered }) {
  switch (rendered.kind) {
    case "svg":
      return (
        <div className="flex justify-center">
          <div
            className="inline-block rounded-lg bg-white p-4 shadow-sm [&>svg]:block [&>svg]:h-auto [&>svg]:w-full [&>svg]:max-w-[260px]"
            // SVG 由本地纯函数生成（qrcode crate），来源可信，无外部脚本。
            dangerouslySetInnerHTML={{ __html: rendered.svg }}
          />
        </div>
      );
    case "color":
      return (
        <div className="flex flex-col gap-4">
          <div
            className="h-28 w-full rounded-xl border border-[var(--color-border)] shadow-inner"
            style={{ backgroundColor: rendered.hex }}
          />
          <ResultFields fields={rendered.fields} />
        </div>
      );
    case "sections":
      return (
        <div className="flex flex-col gap-3">
          {rendered.sections.map((s, i) => (
            <div
              key={i}
              className="overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]"
            >
              <div className="flex items-center justify-between border-b border-[var(--color-border)] px-3 py-1.5">
                <span className="text-xs font-medium uppercase tracking-wide text-[var(--color-accent)]">
                  {s.title}
                </span>
                <CopyButton text={s.body} iconOnly />
              </div>
              <pre className="font-mono overflow-auto whitespace-pre-wrap px-3 py-2 text-sm leading-relaxed text-[var(--color-text)]">
                {s.body}
              </pre>
            </div>
          ))}
        </div>
      );
    case "fields":
      return <ResultFields fields={rendered.fields} />;
    default:
      return null;
  }
}
