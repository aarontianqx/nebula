import { Eraser } from "lucide-react";
import { countLabel } from "../../lib/text";

interface Props {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  /** 标题（默认「输入」）。 */
  label?: string;
  /** 右侧额外操作（如「示例」按钮）。 */
  actions?: React.ReactNode;
}

/** 带标题栏的多行输入区：含字符数、清空。 */
export function InputArea({ value, onChange, placeholder, label = "输入", actions }: Props) {
  return (
    <div className="flex h-full flex-col overflow-hidden bg-[var(--color-bg)]">
      <div className="flex items-center justify-between px-4 py-2">
        <span className="text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
          {label}
        </span>
        <div className="flex items-center gap-2">
          {actions}
          {value && (
            <>
              <span className="text-xs tabular-nums text-[var(--color-text-faint)]">
                {countLabel(value)}
              </span>
              <button
                onClick={() => onChange("")}
                title="清空"
                className="flex items-center gap-1 rounded p-1 text-xs text-[var(--color-text-muted)] transition-colors hover:text-[var(--color-text)]"
              >
                <Eraser size={13} />
              </button>
            </>
          )}
        </div>
      </div>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder ?? "在此粘贴或输入…"}
        spellCheck={false}
        className="font-mono flex-1 resize-none bg-transparent px-4 pb-4 text-sm leading-relaxed text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-faint)]"
      />
    </div>
  );
}
