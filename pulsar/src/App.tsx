import { useEffect } from "react";
import { DetectBar } from "./components/DetectBar";
import { Sidebar } from "./components/Sidebar";
import { ToolPanel } from "./components/ToolPanel";
import { useRegistryStore } from "./stores/registryStore";

export default function App() {
  const { fetchTools, error, loading } = useRegistryStore();

  useEffect(() => {
    fetchTools();
  }, [fetchTools]);

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
    </div>
  );
}
