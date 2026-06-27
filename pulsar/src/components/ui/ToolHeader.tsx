import { Info } from "lucide-react";
import type { ParamSpec, ParamValue, ToolDescriptor } from "../../types/tool";
import { ParamControl } from "../ParamControl";

interface HeaderProps {
  tool: ToolDescriptor;
  /** 标题右侧操作（如示例按钮）。 */
  actions?: React.ReactNode;
}

export function ToolHeader({ tool, actions }: HeaderProps) {
  return (
    <header className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-3.5">
      <div className="flex items-center gap-1.5">
        <h1 className="text-[15px] font-semibold text-[var(--color-text)]">{tool.name}</h1>
        {tool.description && <InfoTip text={tool.description} />}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}

/** 标题旁的信息按钮：悬停（或聚焦）展示工具描述。 */
function InfoTip({ text }: { text: string }) {
  return (
    <span className="group relative inline-flex">
      <button
        type="button"
        tabIndex={0}
        aria-label={text}
        className="flex items-center text-[var(--color-text-faint)] transition-colors hover:text-[var(--color-text-muted)]"
      >
        <Info size={14} />
      </button>
      <span
        role="tooltip"
        className="pointer-events-none absolute left-0 top-full z-20 mt-1.5 w-max max-w-sm rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-2)] px-3 py-2 text-[13px] leading-relaxed text-[var(--color-text-muted)] opacity-0 shadow-lg transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100"
      >
        {text}
      </span>
    </span>
  );
}

interface ParamsBarProps {
  specs: ParamSpec[];
  params: Record<string, ParamValue>;
  setParam: (key: string, value: ParamValue) => void;
}

/** 横向参数条（transform / inspect 顶部）。 */
export function ParamsBar({ specs, params, setParam }: ParamsBarProps) {
  if (specs.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-3 border-b border-[var(--color-border)] bg-[var(--color-surface)]/40 px-6 py-3">
      {specs.map((spec) => (
        <ParamControl
          key={spec.key}
          spec={spec}
          value={params[spec.key]}
          onChange={(v) => setParam(spec.key, v)}
        />
      ))}
    </div>
  );
}
