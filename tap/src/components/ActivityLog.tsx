import { useEffect, useRef } from "react";

import { useLogStore } from "../stores/logStore";

export function ActivityLog() {
  const logs = useLogStore((s) => s.logs);
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
            logs.slice(-30).map((entry, idx) => (
              <div key={idx} className="log-entry">
                <span className="log-time">{entry.time}</span>
                <span className="log-msg">{entry.message}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </>
  );
}
