import { inputHint, isSingleLineInput } from "../../lib/layouts";
import { OutputView } from "../OutputView";
import { ExampleButton } from "../ui/ExampleButton";
import { RunButton } from "../ui/RunButton";
import { ParamsBar, ToolHeader } from "../ui/ToolHeader";
import type { LayoutProps } from "./types";

/**
 * Inspect：紧凑输入 → 结构化字段卡。
 * 输入区不再是占满半屏的大文本框，而是贴合内容的单行框/小文本域；
 * 输出由 OutputView 渲染为字段卡 / 分段 / 色块，主区域留给结果。
 */
export function InspectLayout(p: LayoutProps) {
  const singleLine = isSingleLineInput(p.tool.id);
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (singleLine && e.key === "Enter") {
      e.preventDefault();
      p.run();
    }
  };

  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <ToolHeader
        tool={p.tool}
        actions={<ExampleButton toolId={p.tool.id} onFill={p.setInput} />}
      />
      <ParamsBar specs={p.tool.params} params={p.params} setParam={p.setParam} />

      {/* 输入区：贴合内容尺寸 */}
      <div className="border-b border-[var(--color-border)] px-6 py-4">
        {singleLine ? (
          <input
            value={p.input}
            onChange={(e) => p.setInput(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={inputHint(p.tool.id)}
            spellCheck={false}
            autoFocus
            className="font-mono w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3.5 py-2.5 text-sm text-[var(--color-text)] outline-none transition-colors focus:border-[var(--color-accent)] placeholder:text-[var(--color-text-faint)]"
          />
        ) : (
          <textarea
            value={p.input}
            onChange={(e) => p.setInput(e.target.value)}
            placeholder={inputHint(p.tool.id)}
            spellCheck={false}
            rows={4}
            className="font-mono w-full resize-y rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3.5 py-2.5 text-sm leading-relaxed text-[var(--color-text)] outline-none transition-colors focus:border-[var(--color-accent)] placeholder:text-[var(--color-text-faint)]"
          />
        )}
      </div>

      {/* 输出区：主角 */}
      <div className="min-h-0 flex-1 overflow-hidden">
        <OutputView
          tool={p.tool}
          output={p.output}
          error={p.error}
          placeholder="输入后结果会显示在这里"
        />
      </div>

      {!p.autoRun && (
        <footer className="flex items-center justify-end border-t border-[var(--color-border)] px-6 py-3">
          <RunButton onClick={p.run} running={p.running} />
        </footer>
      )}
    </section>
  );
}
