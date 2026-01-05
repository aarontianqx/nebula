import { useSessionStore, SessionState } from "../../stores/sessionStore";
import { Power, Monitor } from "lucide-react";

const stateColors: Record<SessionState, string> = {
  Idle: "bg-gray-500",
  Starting: "bg-[var(--color-warning)]",
  LoggingIn: "bg-orange-500",
  Ready: "bg-[var(--color-success)]",
  ScriptRunning: "bg-[var(--color-accent)]",
  Stopped: "bg-[var(--color-error)]",
};

const stateLabels: Record<SessionState, string> = {
  Idle: "Idle",
  Starting: "Starting...",
  LoggingIn: "Logging in...",
  Ready: "Ready",
  ScriptRunning: "Running",
  Stopped: "Stopped",
};

export default function SessionList() {
  const {
    sessions,
    selectedSessionId,
    selectSession,
    stopSession,
  } = useSessionStore();

  if (sessions.length === 0) {
    return (
      <div className="p-4 text-center text-[var(--color-text-muted)]">
        <Monitor className="w-8 h-8 mx-auto mb-2 opacity-50" />
        <p className="text-sm">No active sessions</p>
        <p className="text-xs mt-1">Select an account and click Run</p>
      </div>
    );
  }

  return (
    <div className="p-2 space-y-1">
      {sessions.map((session) => {
        const isSelected = selectedSessionId === session.id;
        const isRunning = session.state === "ScriptRunning" || session.state === "Ready";

        return (
          <div
            key={session.id}
            onClick={() => selectSession(session.id)}
            className={`
              relative p-3 rounded-r-md cursor-pointer transition-all
              ${isSelected
                ? "bg-[var(--color-accent)]/10 text-[var(--color-accent)] border-l-4 border-[var(--color-accent)]"
                : "bg-[var(--color-bg-surface)] hover:bg-[var(--color-bg-hover)] border-l-4 border-transparent"
              }
            `}
          >
            <div className="flex items-center justify-between">
              <div className="flex-1 min-w-0">
                <div className={`text-sm font-medium truncate ${isSelected ? "" : "text-[var(--color-text-primary)]"}`}>
                  {session.display_name}
                </div>
                <div className="flex items-center gap-2 mt-1">
                  <span
                    className={`w-2 h-2 rounded-full ${stateColors[session.state]} ${isRunning ? "animate-pulse" : ""}`}
                  />
                  <span className={`text-xs ${isSelected ? "text-[var(--color-accent)]/75" : "text-[var(--color-text-secondary)]"}`}>
                    {stateLabels[session.state]}
                  </span>
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  stopSession(session.id);
                }}
                className={`p-1.5 rounded transition-all ${isSelected
                  ? "hover:bg-[var(--color-accent)]/20 text-[var(--color-accent)]"
                  : "text-[var(--color-text-muted)] hover:bg-[var(--color-error)]/20 hover:text-[var(--color-error)]"
                  }`}
                title="Stop session"
              >
                <Power size={14} />
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
