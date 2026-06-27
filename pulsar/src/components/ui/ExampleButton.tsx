import { Lightbulb } from "lucide-react";
import { exampleFor } from "../../lib/layouts";

interface Props {
  toolId: string;
  onFill: (example: string) => void;
}

/** 「示例」按钮：一键填入该工具的演示输入（没有示例时不渲染）。 */
export function ExampleButton({ toolId, onFill }: Props) {
  const example = exampleFor(toolId);
  if (!example) return null;
  return (
    <button
      onClick={() => onFill(example)}
      className="flex items-center gap-1 rounded-md border border-[var(--color-border)] px-2.5 py-1 text-xs text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-accent)] hover:text-[var(--color-accent)]"
    >
      <Lightbulb size={13} />
      示例
    </button>
  );
}
