import { splitGenerateResult } from "../../lib/layouts";
import { CopyButton } from "./CopyButton";

interface Props {
  output: string;
  error: string | null;
  placeholder?: string;
}

/**
 * generate 类结果展示：突出「主结果」（密码 / ID），复制只取主结果，
 * 元信息（长度 / 熵等）作为弱化的辅助说明，不混入复制内容。
 */
export function GenerateResult({ output, error, placeholder }: Props) {
  if (error) {
    return (
      <div className="flex items-start gap-2 rounded-lg border border-[var(--color-danger)]/40 bg-[var(--color-danger-soft)] px-3 py-2.5">
        <pre className="font-mono whitespace-pre-wrap text-sm leading-relaxed text-[var(--color-danger)]">
          {error}
        </pre>
      </div>
    );
  }
  if (!output) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--color-text-faint)]">
        {placeholder ?? "结果会显示在这里"}
      </div>
    );
  }

  const { primary, meta } = splitGenerateResult(output);
  const lines = primary.split("\n").filter((l) => l.length > 0);
  const multi = lines.length > 1;

  return (
    <div className="animate-fade-in flex flex-col gap-3">
      {/* 主结果卡 */}
      <div className="overflow-hidden rounded-xl border border-[var(--color-border-strong)] bg-[var(--color-surface)]">
        <div className="flex items-center justify-between border-b border-[var(--color-border)] px-3 py-1.5">
          <span className="text-xs font-medium uppercase tracking-wide text-[var(--color-accent)]">
            {multi ? `结果 · ${lines.length}` : "结果"}
          </span>
          <CopyButton text={primary} label={multi ? "复制全部" : "复制"} />
        </div>

        {multi ? (
          <div className="flex flex-col divide-y divide-[var(--color-border)]">
            {lines.map((line, i) => (
              <div key={i} className="group flex items-center justify-between gap-3 px-3 py-1.5">
                <code className="font-mono select-all break-all text-sm text-[var(--color-text)]">
                  {line}
                </code>
                <span className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100">
                  <CopyButton text={line} iconOnly />
                </span>
              </div>
            ))}
          </div>
        ) : (
          <div className="px-4 py-4">
            <code className="font-mono select-all break-all text-lg font-medium tracking-wide text-[var(--color-text)]">
              {primary}
            </code>
          </div>
        )}
      </div>

      {/* 元信息（弱化，不参与复制） */}
      {meta.length > 0 && (
        <div className="flex flex-wrap gap-x-5 gap-y-1.5 px-1 text-xs text-[var(--color-text-muted)]">
          {meta.map((f, i) => (
            <span key={i}>
              <span className="text-[var(--color-text-faint)]">{f.label}:</span> {f.value}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
