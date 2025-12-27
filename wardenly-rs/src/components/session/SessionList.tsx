import { useSessionStore, SessionState } from "../../stores/sessionStore";
import { Square, Monitor } from "lucide-react";

const stateColors: Record<SessionState, string> = {
  Idle: "bg-gray-500",
  Starting: "bg-yellow-500",
  LoggingIn: "bg-orange-500",
  Ready: "bg-green-500",
  ScriptRunning: "bg-blue-500",
  Stopped: "bg-red-500",
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
      {sessions.map((session) => (
        <div
          key={session.id}
          onClick={() => selectSession(session.id)}
          className={`p-3 rounded-md cursor-pointer transition-colors ${
            selectedSessionId === session.id
              ? "bg-[var(--color-accent)] text-white"
              : "bg-[var(--color-bg-tertiary)] hover:bg-[var(--color-border)]"
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium truncate">
                {session.display_name}
              </div>
              <div className="flex items-center gap-2 mt-1">
                <span
                  className={`w-2 h-2 rounded-full ${stateColors[session.state]}`}
                />
                <span className="text-xs opacity-75">
                  {stateLabels[session.state]}
                </span>
              </div>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                stopSession(session.id);
              }}
              className="p-1.5 rounded hover:bg-black/20 transition-colors"
              title="Stop session"
            >
              <Square size={14} />
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

