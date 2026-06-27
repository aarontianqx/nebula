import { useState } from "react";
import {
  ArrowLeftRight,
  Binary,
  ChevronRight,
  FileCode2,
  Hash,
  Image,
  Search,
  Sparkles,
  Star,
  TestTube2,
  Type,
  type LucideIcon,
} from "lucide-react";
import { useRegistryStore } from "../stores/registryStore";
import { useToolStore } from "../stores/toolStore";
import { useUiStore } from "../stores/uiStore";
import { CATEGORY_LABELS, type Category, type ToolDescriptor } from "../types/tool";

const isMac =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

const CATEGORY_ORDER: Category[] = [
  "converters",
  "encoders",
  "formatters",
  "generators",
  "testers",
  "text",
  "graphic",
  "reference",
];

const CATEGORY_ICON: Record<Category, LucideIcon> = {
  converters: ArrowLeftRight,
  encoders: Binary,
  formatters: FileCode2,
  generators: Hash,
  testers: TestTube2,
  text: Type,
  graphic: Image,
  reference: Sparkles,
};

export function Sidebar() {
  const { query, setQuery, byCategory, favoriteTools } = useRegistryStore();
  const grouped = byCategory();
  const favorites = favoriteTools();
  const searching = query.trim().length > 0;

  const [collapsed, setCollapsed] = useState<Set<Category>>(new Set());
  const toggle = (cat: Category) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });

  return (
    <aside className="flex h-full w-64 flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)]">
      {/* 品牌头 */}
      <div className="flex items-center px-4 pb-3 pt-4">
        <div className="text-brand-gradient text-lg font-bold tracking-tight">Pulsar</div>
      </div>

      {/* 搜索 */}
      <div className="px-3 pb-2">
        <div className="flex items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5 focus-within:border-[var(--color-accent)]">
          <Search size={14} className="text-[var(--color-text-muted)]" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索工具…"
            className="w-full bg-transparent text-sm text-[var(--color-text)] outline-none placeholder:text-[var(--color-text-faint)]"
          />
          <button
            onClick={() => useUiStore.getState().openPalette()}
            title="命令面板"
            className="kbd shrink-0 transition-colors hover:text-[var(--color-text)]"
          >
            {isMac ? "⌘K" : "Ctrl K"}
          </button>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 pb-4">
        {/* 收藏（搜索时隐藏） */}
        {!searching && favorites.length > 0 && (
          <div className="mb-3">
            <div className="flex items-center gap-1.5 px-2 py-1 text-[11px] font-semibold uppercase tracking-wider text-[var(--color-text-faint)]">
              <Star size={11} className="fill-[var(--color-warning)] text-[var(--color-warning)]" />
              收藏
            </div>
            {favorites.map((tool) => (
              <ToolItem key={tool.id} tool={tool} />
            ))}
          </div>
        )}

        {CATEGORY_ORDER.map((cat) => {
          const tools = grouped.get(cat);
          if (!tools || tools.length === 0) return null;
          const isCollapsed = !searching && collapsed.has(cat);
          const Icon = CATEGORY_ICON[cat];
          return (
            <div key={cat} className="mb-1.5">
              <button
                onClick={() => toggle(cat)}
                className="group flex w-full items-center gap-1.5 rounded px-2 py-1 text-[11px] font-semibold uppercase tracking-wider text-[var(--color-text-faint)] transition-colors hover:text-[var(--color-text-muted)]"
              >
                <ChevronRight
                  size={12}
                  className={[
                    "transition-transform",
                    isCollapsed ? "" : "rotate-90",
                  ].join(" ")}
                />
                <Icon size={12} />
                <span className="flex-1 text-left">{CATEGORY_LABELS[cat]}</span>
                <span className="text-[var(--color-text-faint)] opacity-0 group-hover:opacity-100">
                  {tools.length}
                </span>
              </button>
              {!isCollapsed && tools.map((tool) => <ToolItem key={tool.id} tool={tool} />)}
            </div>
          );
        })}

        {searching && grouped.size === 0 && (
          <div className="px-3 py-8 text-center text-sm text-[var(--color-text-faint)]">
            没有匹配的工具
          </div>
        )}
      </nav>
    </aside>
  );
}

function ToolItem({ tool }: { tool: ToolDescriptor }) {
  const { active, selectTool } = useToolStore();
  const { isFavorite, toggleFavorite } = useRegistryStore();
  const selected = active?.id === tool.id;
  const fav = isFavorite(tool.id);

  return (
    <div
      className={[
        "group relative flex items-center rounded-md transition-colors",
        selected
          ? "bg-[var(--color-accent-soft)]"
          : "hover:bg-[var(--color-surface-hover)]",
      ].join(" ")}
    >
      {/* 选中态左侧竖条 */}
      {selected && (
        <span className="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-r bg-[var(--color-accent)]" />
      )}
      <button
        onClick={() => selectTool(tool)}
        className={[
          "min-w-0 flex-1 truncate py-1.5 pl-3 pr-1 text-left text-[13px] transition-colors",
          selected ? "font-medium text-[var(--color-accent)]" : "text-[var(--color-text)]",
        ].join(" ")}
      >
        {tool.name}
      </button>
      <button
        onClick={() => toggleFavorite(tool.id)}
        title={fav ? "取消收藏" : "收藏"}
        className={[
          "mr-1.5 shrink-0 rounded p-1 transition-all",
          fav
            ? "text-[var(--color-warning)]"
            : "text-[var(--color-text-faint)] opacity-0 hover:text-[var(--color-warning)] group-hover:opacity-100",
        ].join(" ")}
      >
        <Star size={13} className={fav ? "fill-[var(--color-warning)]" : ""} />
      </button>
    </div>
  );
}
