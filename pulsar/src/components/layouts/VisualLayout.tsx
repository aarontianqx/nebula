import { QrResult } from "../ui/QrResult";
import { RunButton } from "../ui/RunButton";
import { ParamsBar, ToolHeader } from "../ui/ToolHeader";
import type { LayoutProps } from "./types";

/** Visual：文本/参数 → 渲染输出（二维码）。 */
export function VisualLayout(p: LayoutProps) {
  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <ToolHeader tool={p.tool} />
      <ParamsBar specs={p.tool.params} params={p.params} setParam={p.setParam} />

      <div className="border-b border-[var(--color-border)] px-6 py-4">
        <textarea
          value={p.input}
          onChange={(e) => p.setInput(e.target.value)}
          placeholder="要编码的文本 / URL"
          spellCheck={false}
          rows={3}
          autoFocus
          className="font-mono w-full resize-y rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3.5 py-2.5 text-sm leading-relaxed text-[var(--color-text)] outline-none transition-colors focus:border-[var(--color-accent)] placeholder:text-[var(--color-text-faint)]"
        />
      </div>

      <div className="min-h-0 flex-1 overflow-hidden">
        <QrResult output={p.output} error={p.error} placeholder="生成的二维码会显示在这里" />
      </div>

      {!p.autoRun && (
        <footer className="flex items-center justify-end border-t border-[var(--color-border)] px-6 py-3">
          <RunButton onClick={p.run} running={p.running} label="生成" />
        </footer>
      )}
    </section>
  );
}
