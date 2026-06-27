import { useState } from "react";
import { Check, Copy } from "lucide-react";

interface Props {
  text: string;
  /** 仅图标（用于字段卡等紧凑场景）。 */
  iconOnly?: boolean;
  label?: string;
  className?: string;
}

/** 统一的复制按钮：点击后短暂显示「已复制」。 */
export function CopyButton({ text, iconOnly = false, label = "复制", className = "" }: Props) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    if (!text) return;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  };

  return (
    <button
      onClick={copy}
      disabled={!text}
      title={iconOnly ? label : undefined}
      className={[
        "flex items-center gap-1 rounded text-xs text-[var(--color-text-muted)] transition-colors hover:text-[var(--color-text)] disabled:opacity-40",
        iconOnly ? "p-1" : "px-2 py-0.5",
        className,
      ].join(" ")}
    >
      {copied ? <Check size={13} className="text-[var(--color-success)]" /> : <Copy size={13} />}
      {!iconOnly && (copied ? "已复制" : label)}
    </button>
  );
}
