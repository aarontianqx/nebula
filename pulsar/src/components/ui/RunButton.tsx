import { Play, RefreshCw } from "lucide-react";

interface Props {
  onClick: () => void;
  running: boolean;
  /** 文案（默认「运行」）。生成类用「生成」。 */
  label?: string;
  /** 显示 ⌘↵ 提示。 */
  hint?: boolean;
  /** 重新生成样式（用 RefreshCw 图标）。 */
  regenerate?: boolean;
}

const isMac =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

export function RunButton({ onClick, running, label = "运行", hint = true, regenerate }: Props) {
  return (
    <button
      onClick={onClick}
      disabled={running}
      className="flex items-center gap-2 rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white shadow-sm transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
    >
      {regenerate ? <RefreshCw size={15} /> : <Play size={15} />}
      {running ? "运行中…" : label}
      {hint && !running && (
        <span className="kbd ml-1 !border-white/25 !bg-white/15 !text-white/80">
          {isMac ? "⌘↵" : "Ctrl+↵"}
        </span>
      )}
    </button>
  );
}
