import { useEffect, useState } from "react";
import { Settings, Play, Square } from "lucide-react";
import { useAccountStore } from "../../stores/accountStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useTauriEvents } from "../../hooks/useTauriEvents";
import ManagementDialog from "../dialogs/ManagementDialog";
import SessionList from "../session/SessionList";
import CanvasWindow from "../canvas/CanvasWindow";
import ScriptControls from "../session/ScriptControls";

function MainWindow() {
  const { accounts, fetchAccounts, fetchGroups } = useAccountStore();
  const {
    sessions,
    selectedSessionId,
    startSession,
    stopAllSessions,
    loading,
  } = useSessionStore();
  const [showManagement, setShowManagement] = useState(false);
  const [selectedAccountId, setSelectedAccountId] = useState<string>("");

  // Initialize Tauri event listeners
  useTauriEvents();

  useEffect(() => {
    fetchAccounts();
    fetchGroups();
  }, [fetchAccounts, fetchGroups]);

  const handleRun = async () => {
    if (!selectedAccountId) return;
    try {
      await startSession(selectedAccountId);
    } catch (error) {
      console.error("Failed to start session:", error);
    }
  };

  const handleStopAll = async () => {
    await stopAllSessions();
  };

  // Check if selected account already has a session
  const hasSessionForAccount = sessions.some(
    (s) => s.account_id === selectedAccountId
  );

  return (
    <div className="flex flex-col h-screen bg-[var(--color-bg-primary)]">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-3 bg-[var(--color-bg-secondary)] border-b border-[var(--color-border)]">
        <div className="flex items-center gap-4">
          <h1 className="text-lg font-semibold text-[var(--color-text-primary)]">
            Wardenly
          </h1>

          {/* Account selector */}
          <select
            value={selectedAccountId}
            onChange={(e) => setSelectedAccountId(e.target.value)}
            className="px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border border-[var(--color-border)] focus:outline-none focus:border-[var(--color-accent)]"
          >
            <option value="">Select Account</option>
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.server_id} - {account.role_name}
              </option>
            ))}
          </select>

          {/* Run button */}
          <button
            onClick={handleRun}
            disabled={!selectedAccountId || hasSessionForAccount || loading}
            className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-accent)] text-white hover:bg-[var(--color-accent-hover)] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <Play size={14} />
            Run
          </button>

          {/* Stop All button */}
          {sessions.length > 0 && (
            <button
              onClick={handleStopAll}
              className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-error)] text-white hover:opacity-80 transition-opacity"
            >
              <Square size={14} />
              Stop All
            </button>
          )}

          <span className="text-sm text-[var(--color-text-secondary)]">
            {sessions.length} sessions
          </span>

          {/* Divider */}
          <div className="w-px h-6 bg-[var(--color-border)]" />

          {/* Script Controls */}
          <ScriptControls
            sessionId={selectedSessionId}
            sessionState={
              sessions.find((s) => s.id === selectedSessionId)?.state || null
            }
          />
        </div>

        <button
          onClick={() => setShowManagement(true)}
          className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] hover:bg-[var(--color-border)] transition-colors"
        >
          <Settings size={16} />
          Manage
        </button>
      </div>

      {/* Main Content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Session List */}
        <div className="w-64 flex-shrink-0 bg-[var(--color-bg-secondary)] border-r border-[var(--color-border)] overflow-y-auto">
          <SessionList />
        </div>

        {/* Canvas Panel */}
        <div className="flex-1 flex items-center justify-center p-4 overflow-auto">
          {selectedSessionId ? (
            <CanvasWindow sessionId={selectedSessionId} />
          ) : (
            <div className="text-center text-[var(--color-text-muted)]">
              <p className="text-lg">Select a session to view</p>
              <p className="text-sm mt-2">
                Or select an account and click Run to start
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Management Dialog */}
      {showManagement && (
        <ManagementDialog onClose={() => setShowManagement(false)} />
      )}
    </div>
  );
}

export default MainWindow;
