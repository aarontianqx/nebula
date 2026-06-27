import { useEffect, useState } from "react";
import { OutputView } from "../OutputView";
import { RunButton } from "../ui/RunButton";
import { ToolHeader } from "../ui/ToolHeader";
import type { LayoutProps } from "./types";

const SEP = "\n=====\n";

/** 文本 Diff：左右两段独立输入，运行时用 `=====` 分隔合成后端约定的单一输入。 */
export function DiffPanes(p: LayoutProps) {
  const [left, setLeft] = useState("");
  const [right, setRight] = useState("");

  // 把两段合成后端约定格式写回 store.input。
  useEffect(() => {
    p.setInput(`${left}${SEP}${right}`);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [left, right]);

  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <ToolHeader tool={p.tool} />

      <div className="grid flex-1 grid-cols-2 gap-px overflow-hidden border-b border-[var(--color-border)] bg-[var(--color-border)]">
        <DiffInput label="原文本" value={left} onChange={setLeft} />
        <DiffInput label="对比文本" value={right} onChange={setRight} />
      </div>

      <div className="min-h-0 flex-[1.2] overflow-hidden">
        <OutputView
          tool={p.tool}
          output={p.output}
          error={p.error}
          placeholder="差异结果会显示在这里（运行后）"
        />
      </div>

      <footer className="flex items-center justify-end border-t border-[var(--color-border)] px-6 py-3">
        <RunButton onClick={p.run} running={p.running} label="对比" />
      </footer>
    </section>
  );
}

function DiffInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="flex flex-col overflow-hidden bg-[var(--color-bg)]">
      <div className="px-4 py-2 text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
        {label}
      </div>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="在此粘贴或输入…"
        spellCheck={false}
        className="font-mono flex-1 resize-none bg-transparent px-4 pb-4 text-sm leading-relaxed text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-faint)]"
      />
    </div>
  );
}
