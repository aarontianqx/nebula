import { useEffect, useRef } from "react";
import { useToolStore } from "../stores/toolStore";
import { archetypeOf, isAutoRun, runsOnEmpty } from "../lib/layouts";
import { GenerateLayout } from "./layouts/GenerateLayout";
import { InspectLayout } from "./layouts/InspectLayout";
import { QueryLayout } from "./layouts/QueryLayout";
import { TransformLayout } from "./layouts/TransformLayout";
import { VisualLayout } from "./layouts/VisualLayout";
import type { LayoutProps } from "./layouts/types";

export function ToolPanel() {
  const { active, input, params, output, error, running, setInput, setParam, run, clearOutput } =
    useToolStore();

  const autoRun = active ? isAutoRun(active.id) : false;

  // 轻量工具：输入/参数变化后防抖自动运行。
  // 空输入通常清空输出（不报错）；但「依赖当前时间」类工具空输入也运行（→ now）。
  const timer = useRef<number | null>(null);
  useEffect(() => {
    if (!active || !autoRun) return;
    if (timer.current) window.clearTimeout(timer.current);
    if (input.trim() === "" && !runsOnEmpty(active.id)) {
      clearOutput();
      return;
    }
    timer.current = window.setTimeout(() => run(), 200);
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [input, params, active?.id, autoRun]);

  // 全局快捷键：Cmd/Ctrl+Enter 运行。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        if (active && !running) run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active?.id, running]);

  if (!active) {
    return (
      <div className="flex h-full flex-1 items-center justify-center text-[var(--color-text-muted)]">
        从左侧选择一个工具开始
      </div>
    );
  }

  const layoutProps: LayoutProps = {
    tool: active,
    input,
    params,
    output,
    error,
    running,
    autoRun,
    setInput,
    setParam,
    run,
  };

  switch (archetypeOf(active.id)) {
    case "inspect":
      return <InspectLayout {...layoutProps} />;
    case "generate":
      return <GenerateLayout {...layoutProps} />;
    case "query":
      return <QueryLayout {...layoutProps} />;
    case "visual":
      return <VisualLayout {...layoutProps} />;
    default:
      return <TransformLayout {...layoutProps} />;
  }
}
