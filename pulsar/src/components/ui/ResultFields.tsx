import type { OutputField } from "../../lib/layouts";
import { CopyButton } from "./CopyButton";

interface Props {
  fields: OutputField[];
}

/**
 * 结构化字段卡：每个「标签 + 值」一行，值等宽、可单独复制。
 * 用于 inspect 类工具（时间戳 / 进制 / 颜色 / 哈希 …），
 * 把原本一坨文本变成可逐项扫读、逐项复制的结果。
 */
export function ResultFields({ fields }: Props) {
  return (
    <div className="flex flex-col gap-1.5">
      {fields.map((f, i) =>
        f.label ? (
          <div
            key={i}
            className="group flex items-center gap-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 transition-colors hover:border-[var(--color-border-strong)]"
          >
            <span className="w-28 shrink-0 text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
              {f.label}
            </span>
            <span className="font-mono min-w-0 flex-1 break-all text-sm text-[var(--color-text)]">
              {f.value}
            </span>
            <span className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100">
              <CopyButton text={f.value} iconOnly />
            </span>
          </div>
        ) : (
          <div key={i} className="font-mono px-3 text-sm text-[var(--color-text-muted)]">
            {f.value}
          </div>
        ),
      )}
    </div>
  );
}
