import { ParamControl } from "../ParamControl";
import { GenerateResult } from "../ui/GenerateResult";
import { RunButton } from "../ui/RunButton";
import { ToolHeader } from "../ui/ToolHeader";
import type { LayoutProps } from "./types";

/**
 * Generate：没有输入，主体是参数表单。
 * 参数卡 + 醒目的「生成」按钮，结果在下方（可复制 / 重新生成）。
 */
export function GenerateLayout(p: LayoutProps) {
  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <ToolHeader tool={p.tool} />

      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-6 py-5">
        {/* 参数表单卡 + 生成按钮 */}
        <div className="flex flex-wrap items-end gap-x-8 gap-y-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-5 py-4">
          {p.tool.params.map((spec) => (
            <ParamControl
              key={spec.key}
              spec={spec}
              value={p.params[spec.key]}
              onChange={(v) => p.setParam(spec.key, v)}
              stacked
            />
          ))}
          <RunButton
            onClick={p.run}
            running={p.running}
            label="生成"
            regenerate={Boolean(p.output)}
          />
        </div>

        {/* 结果 */}
        <div className="min-h-0 flex-1 overflow-auto">
          <GenerateResult output={p.output} error={p.error} placeholder="点击「生成」获取结果" />
        </div>
      </div>
    </section>
  );
}
