import { useEffect, useRef } from "react";

import { useEngineStore } from "../stores/engineStore";

export function ActivityLog() {
  const logs = useEngineStore((s) => s.logs);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs]);

  return (
    <>
      <h3>Activity Log</h3>
      <div className="card log-card">
        <div className="log-container" ref={containerRef}>
          {logs.length === 0 ? (
            <div className="log-empty">No activity yet</div>
          ) : (
            logs.slice(-30).map((log, idx) => (
              <div key={idx} className="log-entry">
                <span className="log-time">{log.time}</span>
                <span className="log-msg">{log.message}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </>
  );
}
