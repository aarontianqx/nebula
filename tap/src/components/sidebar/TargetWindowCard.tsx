import { useCallback, useEffect, useState } from "react";

import { api } from "../../lib/ipc";
import type { WindowInfoResponse } from "../../lib/types";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";

export function TargetWindowCard() {
  const engineState = useEngineStore((s) => s.engineState);
  const targetWindowMatched = useEngineStore((s) => s.targetWindowMatched);
  const isIdle = engineState === "Idle";
  const active = engineState === "Running" || engineState === "Paused";

  const targetTitle = useDocumentStore((s) => s.targetTitle);
  const pauseWhenUnfocused = useDocumentStore((s) => s.pauseWhenUnfocused);
  const editable = useDocumentStore((s) => s.editable);
  const doc = useDocumentStore.getState;

  const [windowList, setWindowList] = useState<WindowInfoResponse[]>([]);
  const disabled = !isIdle || !editable;

  const refresh = useCallback(async () => {
    try {
      setWindowList(await api.listWindows());
    } catch {
      // ignore; the list stays as-is
    }
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [refresh]);

  return (
    <>
      <h3>Target Window</h3>
      <div className="card">
        <div className="field">
          <label className="label">Window</label>
          <select
            value={targetTitle}
            onChange={(e) => {
              const win = windowList.find((w) => w.title === e.target.value);
              doc().setTargetTitle(e.target.value, win?.process_name ?? "");
            }}
            disabled={disabled}
            className="input"
          >
            <option value="">Any window</option>
            {windowList.map((w) => (
              <option key={w.handle} value={w.title}>
                {w.title.slice(0, 40)} ({w.process_name})
              </option>
            ))}
          </select>
          <button className="btn btn-sm" onClick={refresh} disabled={disabled} style={{ marginTop: 4 }}>
            Refresh
          </button>
        </div>
        <div className="field">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={pauseWhenUnfocused}
              onChange={(e) => doc().setPauseWhenUnfocused(e.target.checked)}
              disabled={disabled}
            />
            Pause when target window not focused
          </label>
        </div>
        {!targetWindowMatched && active && <div className="warning-box">Target window not focused</div>}
      </div>
    </>
  );
}
