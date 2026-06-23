import { useState } from "react";
import { Copy, Check, Play } from "lucide-react";
import { useToolStore } from "../stores/toolStore";
import { ParamControl } from "./ParamControl";

export function ToolPanel() {
  const { active, input, params, output, error, running, setInput, setParam, run } =
    useToolStore();
  const [copied, setCopied] = useState(false);

  if (!active) {
    return (
      <div className="flex h-full flex-1 items-center justify-center text-[var(--color-text-muted)]">
        从左侧选择一个工具开始
      </div>
    );
  }

  const copyOutput = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  };

  return (
    <section className="flex h-full flex-1 flex-col overflow-hidden">
      <header className="border-b border-[var(--color-border)] px-6 py-4">
        <h1 className="text-base font-semibold">{active.name}</h1>
        <p className="mt-0.5 text-sm text-[var(--color-text-muted)]">
          {active.description}
        </p>
      </header>

      {active.params.length > 0 && (
        <div className="flex flex-wrap items-center gap-4 border-b border-[var(--color-border)] px-6 py-3">
          {active.params.map((spec) => (
            <ParamControl
              key={spec.key}
              spec={spec}
              value={params[spec.key]}
              onChange={(v) => setParam(spec.key, v)}
            />
          ))}
        </div>
      )}

      <div className="grid flex-1 grid-cols-2 gap-px overflow-hidden bg-[var(--color-border)]">
        <div className="flex flex-col overflow-hidden bg-[var(--color-bg)]">
          <div className="px-4 py-2 text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
            输入
          </div>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="在此粘贴或输入…"
            spellCheck={false}
            className="font-mono flex-1 resize-none bg-transparent px-4 pb-4 text-sm leading-relaxed outline-none placeholder:text-[var(--color-text-muted)]"
          />
        </div>

        <div className="flex flex-col overflow-hidden bg-[var(--color-bg)]">
          <div className="flex items-center justify-between px-4 py-2">
            <span className="text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
              输出
            </span>
            <button
              onClick={copyOutput}
              disabled={!output}
              className="flex items-center gap-1 rounded px-2 py-0.5 text-xs text-[var(--color-text-muted)] transition-colors hover:text-[var(--color-text)] disabled:opacity-40"
            >
              {copied ? <Check size={13} /> : <Copy size={13} />}
              {copied ? "已复制" : "复制"}
            </button>
          </div>
          <pre className="font-mono flex-1 overflow-auto whitespace-pre-wrap px-4 pb-4 text-sm leading-relaxed">
            {error ? (
              <span className="text-[var(--color-danger)]">{error}</span>
            ) : (
              output
            )}
          </pre>
        </div>
      </div>

      <footer className="flex items-center justify-end border-t border-[var(--color-border)] px-6 py-3">
        <button
          onClick={run}
          disabled={running}
          className="flex items-center gap-2 rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
        >
          <Play size={15} />
          {running ? "运行中…" : "运行"}
        </button>
      </footer>
    </section>
  );
}
