import { Search } from "lucide-react";
import { ParamControl } from "../ParamControl";
import { OutputView } from "../OutputView";
import { ExampleButton } from "../ui/ExampleButton";
import { RunButton } from "../ui/RunButton";
import { ToolHeader } from "../ui/ToolHeader";
import { DiffPanes } from "./DiffPanes";
import type { LayoutProps } from "./types";

/**
 * Query：查询字段 + 主体文本 → 匹配 / 结果。
 * - 正则 / JSONPath：把字符串参数（pattern / path）提升为顶部醒目查询框，
 *   其余参数（flags）作为选项条，主体文本在下方。
 * - Diff：左右两段文本对照输入。
 */
export function QueryLayout(p: LayoutProps) {
  if (p.tool.id === "testers.diff") {
    return <DiffPanes {...p} />;
  }

  const queryParam = p.tool.params.find((s) => s.kind === "str");
  const otherParams = p.tool.params.filter((s) => s.key !== queryParam?.key);

  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <ToolHeader
        tool={p.tool}
        actions={<ExampleButton toolId={p.tool.id} onFill={p.setInput} />}
      />

      {/* 查询区 */}
      <div className="flex flex-col gap-3 border-b border-[var(--color-border)] px-6 py-4">
        {queryParam && (
          <div className="flex items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 focus-within:border-[var(--color-accent)]">
            <Search size={15} className="shrink-0 text-[var(--color-text-muted)]" />
            <input
              value={String(p.params[queryParam.key] ?? "")}
              onChange={(e) => p.setParam(queryParam.key, e.target.value)}
              placeholder={queryParam.label}
              spellCheck={false}
              autoFocus
              className="font-mono w-full bg-transparent text-sm text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-faint)]"
            />
          </div>
        )}
        {otherParams.length > 0 && (
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
            {otherParams.map((spec) => (
              <ParamControl
                key={spec.key}
                spec={spec}
                value={p.params[spec.key]}
                onChange={(v) => p.setParam(spec.key, v)}
              />
            ))}
          </div>
        )}
      </div>

      {/* 主体 + 结果 */}
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-px overflow-hidden bg-[var(--color-border)]">
        <div className="flex flex-col overflow-hidden bg-[var(--color-bg)]">
          <div className="px-4 py-2 text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
            待测文本
          </div>
          <textarea
            value={p.input}
            onChange={(e) => p.setInput(e.target.value)}
            placeholder="在此粘贴或输入…"
            spellCheck={false}
            className="font-mono flex-1 resize-none bg-transparent px-4 pb-4 text-sm leading-relaxed text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-faint)]"
          />
        </div>
        <OutputView tool={p.tool} output={p.output} error={p.error} placeholder="匹配结果会显示在这里" />
      </div>

      {!p.autoRun && (
        <footer className="flex items-center justify-end border-t border-[var(--color-border)] px-6 py-3">
          <RunButton onClick={p.run} running={p.running} />
        </footer>
      )}
    </section>
  );
}
