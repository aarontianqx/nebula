import { inputHint } from "../../lib/layouts";
import { OutputView } from "../OutputView";
import { ExampleButton } from "../ui/ExampleButton";
import { InputArea } from "../ui/InputArea";
import { RunButton } from "../ui/RunButton";
import { ParamsBar, ToolHeader } from "../ui/ToolHeader";
import type { LayoutProps } from "./types";

/** Transform：大文本输入 → 大文本输出（编码 / 格式化 / 批量文本）。 */
export function TransformLayout(p: LayoutProps) {
  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <ToolHeader
        tool={p.tool}
        actions={<ExampleButton toolId={p.tool.id} onFill={p.setInput} />}
      />
      <ParamsBar specs={p.tool.params} params={p.params} setParam={p.setParam} />

      <div className="grid flex-1 grid-cols-2 gap-px overflow-hidden bg-[var(--color-border)]">
        <InputArea
          value={p.input}
          onChange={p.setInput}
          placeholder={inputHint(p.tool.id)}
        />
        <OutputView tool={p.tool} output={p.output} error={p.error} />
      </div>

      {!p.autoRun && (
        <footer className="flex items-center justify-end border-t border-[var(--color-border)] px-6 py-3">
          <RunButton onClick={p.run} running={p.running} />
        </footer>
      )}
    </section>
  );
}
