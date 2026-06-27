import { useEffect } from "react";
import { CommandPalette } from "./components/CommandPalette";
import { DetectBar } from "./components/DetectBar";
import { Sidebar } from "./components/Sidebar";
import { ToolPanel } from "./components/ToolPanel";
import { useRegistryStore } from "./stores/registryStore";
import { useUiStore } from "./stores/uiStore";

export default function App() {
  const { fetchTools, error, loading } = useRegistryStore();
  const { togglePalette } = useUiStore();

  useEffect(() => {
    fetchTools();
  }, [fetchTools]);

  // 全局快捷键：Cmd/Ctrl+K 打开命令面板。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        togglePalette();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePalette]);

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Sidebar />
      <div className="flex h-full flex-1 flex-col overflow-hidden">
        {error ? (
          <div className="flex flex-1 items-center justify-center px-8 text-center text-sm text-[var(--color-text-muted)]">
            无法加载工具列表：{error}
          </div>
        ) : loading ? (
          <div className="flex flex-1 items-center justify-center text-sm text-[var(--color-text-muted)]">
            加载中…
          </div>
        ) : (
          <>
            <DetectBar />
            <ToolPanel />
          </>
        )}
      </div>
      <CommandPalette />
    </div>
  );
}
