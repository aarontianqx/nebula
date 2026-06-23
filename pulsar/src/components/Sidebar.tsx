import { Search } from "lucide-react";
import { useRegistryStore } from "../stores/registryStore";
import { useToolStore } from "../stores/toolStore";
import { CATEGORY_LABELS, type Category } from "../types/tool";

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

export function Sidebar() {
  const { query, setQuery, byCategory } = useRegistryStore();
  const { active, selectTool } = useToolStore();
  const grouped = byCategory();

  return (
    <aside className="flex h-full w-72 flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)]">
      <div className="flex items-center gap-2 px-4 py-4">
        <div className="text-lg font-semibold tracking-tight">Pulsar</div>
        <span className="text-xs text-[var(--color-text-muted)]">本地工具箱</span>
      </div>

      <div className="px-3 pb-3">
        <div className="flex items-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2.5 py-1.5">
          <Search size={15} className="text-[var(--color-text-muted)]" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索工具…"
            className="w-full bg-transparent text-sm outline-none placeholder:text-[var(--color-text-muted)]"
          />
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 pb-4">
        {CATEGORY_ORDER.map((cat) => {
          const tools = grouped.get(cat);
          if (!tools || tools.length === 0) return null;
          return (
            <div key={cat} className="mb-3">
              <div className="px-2 py-1 text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
                {CATEGORY_LABELS[cat]}
              </div>
              {tools.map((tool) => {
                const selected = active?.id === tool.id;
                return (
                  <button
                    key={tool.id}
                    onClick={() => selectTool(tool)}
                    className={[
                      "w-full rounded-md px-2.5 py-1.5 text-left text-sm transition-colors",
                      selected
                        ? "bg-[var(--color-accent)] text-white"
                        : "text-[var(--color-text)] hover:bg-[var(--color-surface-hover)]",
                    ].join(" ")}
                  >
                    {tool.name}
                  </button>
                );
              })}
            </div>
          );
        })}
      </nav>
    </aside>
  );
}
