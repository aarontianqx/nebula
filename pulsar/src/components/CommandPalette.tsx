import { useEffect, useMemo, useRef, useState } from "react";
import { CornerDownLeft, Search } from "lucide-react";
import { useRegistryStore } from "../stores/registryStore";
import { useToolStore } from "../stores/toolStore";
import { useUiStore } from "../stores/uiStore";
import { CATEGORY_LABELS, type ToolDescriptor } from "../types/tool";

/** 命令面板：Cmd/Ctrl+K 模糊搜索全部工具，键盘上下选择、回车跳转。 */
export function CommandPalette() {
  const { paletteOpen, closePalette } = useUiStore();
  const { tools } = useRegistryStore();
  const { selectTool } = useToolStore();

  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // 打开时重置并聚焦。
  useEffect(() => {
    if (paletteOpen) {
      setQuery("");
      setActive(0);
      // 等待渲染后聚焦
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [paletteOpen]);

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return tools;
    return tools.filter(
      (t) =>
        t.name.toLowerCase().includes(q) ||
        t.id.includes(q) ||
        CATEGORY_LABELS[t.category].toLowerCase().includes(q) ||
        t.keywords.some((k) => k.toLowerCase().includes(q)),
    );
  }, [query, tools]);

  // query 变化时把高亮夹回范围内。
  useEffect(() => {
    setActive((a) => Math.min(a, Math.max(0, results.length - 1)));
  }, [results.length]);

  const choose = (tool: ToolDescriptor | undefined) => {
    if (!tool) return;
    selectTool(tool);
    closePalette();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(results[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      closePalette();
    }
  };

  // 高亮项滚动进可视区。
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-idx="${active}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [active]);

  if (!paletteOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 pt-[12vh]"
      onClick={closePalette}
    >
      <div
        className="animate-fade-in flex max-h-[60vh] w-[560px] max-w-[90vw] flex-col overflow-hidden rounded-xl border border-[var(--color-border-strong)] bg-[var(--color-surface)] shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2.5 border-b border-[var(--color-border)] px-4 py-3">
          <Search size={16} className="shrink-0 text-[var(--color-text-muted)]" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="搜索工具…"
            className="w-full bg-transparent text-sm text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-faint)]"
          />
          <span className="kbd">esc</span>
        </div>

        <div ref={listRef} className="flex-1 overflow-y-auto p-1.5">
          {results.length === 0 ? (
            <div className="px-3 py-8 text-center text-sm text-[var(--color-text-faint)]">
              没有匹配的工具
            </div>
          ) : (
            results.map((tool, i) => (
              <button
                key={tool.id}
                data-idx={i}
                onMouseMove={() => setActive(i)}
                onClick={() => choose(tool)}
                className={[
                  "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors",
                  i === active ? "bg-[var(--color-accent-soft)]" : "",
                ].join(" ")}
              >
                <span
                  className={[
                    "flex-1 truncate text-sm",
                    i === active
                      ? "font-medium text-[var(--color-accent)]"
                      : "text-[var(--color-text)]",
                  ].join(" ")}
                >
                  {tool.name}
                </span>
                <span className="shrink-0 text-xs text-[var(--color-text-faint)]">
                  {CATEGORY_LABELS[tool.category]}
                </span>
                {i === active && (
                  <CornerDownLeft size={13} className="shrink-0 text-[var(--color-accent)]" />
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
