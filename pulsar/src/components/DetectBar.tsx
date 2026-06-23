import { useEffect, useRef, useState } from "react";
import { Sparkles } from "lucide-react";
import { api } from "../lib/ipc";
import { useRegistryStore } from "../stores/registryStore";
import { useToolStore } from "../stores/toolStore";
import type { DetectionResult } from "../types/tool";

/** 顶部全局识别栏：粘贴内容 → 自动识别 → 候选工具一键跳转。 */
export function DetectBar() {
  const [text, setText] = useState("");
  const [candidates, setCandidates] = useState<DetectionResult[]>([]);
  const { tools } = useRegistryStore();
  const { selectAndFill } = useToolStore();
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (timer.current) window.clearTimeout(timer.current);
    const trimmed = text.trim();
    if (!trimmed) {
      setCandidates([]);
      return;
    }
    // 去抖：停止输入 250ms 后再识别。
    timer.current = window.setTimeout(async () => {
      try {
        setCandidates(await api.detect(trimmed));
      } catch {
        setCandidates([]);
      }
    }, 250);
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, [text]);

  const jump = (c: DetectionResult) => {
    const tool = tools.find((t) => t.id === c.tool_id);
    if (tool) {
      selectAndFill(tool, text.trim());
      setText("");
      setCandidates([]);
    }
  };

  return (
    <div className="border-b border-[var(--color-border)] bg-[var(--color-surface)] px-6 py-3">
      <div className="flex items-center gap-2">
        <Sparkles size={16} className="shrink-0 text-[var(--color-accent)]" />
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="粘贴任意内容，自动识别工具（JSON / JWT / 时间戳 / Base64 …）"
          spellCheck={false}
          className="font-mono w-full bg-transparent text-sm outline-none placeholder:text-[var(--color-text-muted)]"
        />
      </div>

      {candidates.length > 0 && (
        <div className="mt-2 flex flex-wrap items-center gap-2 pl-6">
          <span className="text-xs text-[var(--color-text-muted)]">识别为：</span>
          {candidates.map((c) => (
            <button
              key={c.tool_id}
              onClick={() => jump(c)}
              className="flex items-center gap-1.5 rounded-full border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1 text-xs transition-colors hover:border-[var(--color-accent)] hover:text-[var(--color-accent)]"
            >
              {c.tool_name}
              <span className="text-[var(--color-text-muted)]">{c.confidence}%</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
