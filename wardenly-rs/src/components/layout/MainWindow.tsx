import { useEffect, useState, useRef } from "react";
import { Settings, Play, Square, Keyboard, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
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
  const [keyboardPassthrough, setKeyboardPassthrough] = useState(false);
  const [spreadToAll, setSpreadToAll] = useState(false);
  // Screencast controls streaming (true = streaming mode, false = stopped)
  // Default: true for better UX - user sees screen immediately after login
  const [screencastEnabled, setScreencastEnabled] = useState(true);
  // Track which session currently has screencast running to avoid duplicate start/stop
  const screencastSessionRef = useRef<string | null>(null);

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

  const toggleKeyboardPassthrough = async () => {
    const newValue = !keyboardPassthrough;
    try {
      await invoke("set_keyboard_passthrough", { enabled: newValue });
      setKeyboardPassthrough(newValue);
    } catch (error) {
      console.error("Failed to toggle keyboard passthrough:", error);
      // Show permission dialog hint on macOS
      if (String(error).includes("accessibility")) {
        alert(
          "Keyboard passthrough requires accessibility permissions. Please enable in System Preferences > Security & Privacy > Privacy > Accessibility."
        );
      }
    }
  };

  // Update active session for input processor when selection changes
  useEffect(() => {
    invoke("set_active_session_for_input", { sessionId: selectedSessionId });
  }, [selectedSessionId]);

  // Manage screencast based on screencast toggle and selected session
  // Screencast ON = start screencast (streaming mode at ~5 FPS)
  // Screencast OFF = stop screencast
  useEffect(() => {
    const manageScreencast = async () => {
      const currentScreencast = screencastSessionRef.current;

      // If screencast is enabled and we have a selected session
      if (screencastEnabled && selectedSessionId) {
        // If currently screencasting a different session, stop it first
        if (currentScreencast && currentScreencast !== selectedSessionId) {
          try {
            await invoke("stop_screencast", { sessionId: currentScreencast });
          } catch {
            // Ignore errors when stopping old screencast
          }
        }

        // Start screencast for the new session if not already running
        if (currentScreencast !== selectedSessionId) {
          try {
            await invoke("start_screencast", { sessionId: selectedSessionId });
            screencastSessionRef.current = selectedSessionId;
          } catch (error) {
            console.error("Failed to start screencast:", error);
          }
        }
      } else {
        // Screencast disabled or no session selected - stop any active screencast
        if (currentScreencast) {
          try {
            await invoke("stop_screencast", { sessionId: currentScreencast });
          } catch {
            // Ignore errors when stopping
          }
          screencastSessionRef.current = null;
        }
      }
    };

    manageScreencast();
  }, [screencastEnabled, selectedSessionId]);

  const handleRefresh = async () => {
    if (!selectedSessionId) return;
    try {
      await invoke("refresh_session", { sessionId: selectedSessionId });
    } catch (error) {
      console.error("Failed to refresh:", error);
    }
  };

  return (
    <div className="flex flex-col h-screen bg-[var(--color-bg-primary)]">
      {/* Toolbar - Two Rows */}
      <div className="bg-[var(--color-bg-secondary)] border-b border-[var(--color-border)]">
        {/* Row 1: Main controls */}
        <div className="flex items-center justify-between px-4 py-2">
          <div className="flex items-center gap-3">
            <h1 className="text-lg font-semibold text-[var(--color-text-primary)]">
              Wardenly
            </h1>

            {/* Account selector */}
            <select
              value={selectedAccountId}
              onChange={(e) => setSelectedAccountId(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border border-[var(--color-border)] focus:outline-none focus:border-[var(--color-accent)] min-w-[160px]"
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
          </div>

          <button
            onClick={() => setShowManagement(true)}
            className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] hover:bg-[var(--color-border)] transition-colors"
          >
            <Settings size={16} />
            Manage
          </button>
        </div>

        {/* Row 2: Script controls and options */}
        <div className="flex items-center justify-between px-4 py-2 border-t border-[var(--color-border)] bg-[var(--color-bg-tertiary)]">
          <div className="flex items-center gap-4">
            {/* Script Controls */}
            <ScriptControls
              sessionId={selectedSessionId}
              sessionState={
                sessions.find((s) => s.id === selectedSessionId)?.state || null
              }
            />

            {/* Divider */}
            <div className="w-px h-5 bg-[var(--color-border)]" />

            {/* Session Controls */}
            <button
              onClick={handleRefresh}
              disabled={!selectedSessionId}
              className="flex items-center gap-1.5 px-2 py-1 text-sm rounded bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] disabled:opacity-50 disabled:cursor-not-allowed transition-colors border border-[var(--color-border)]"
              title="Refresh current session page"
            >
              <RefreshCw size={14} />
              Refresh
            </button>
          </div>

          <div className="flex items-center gap-4">
            {/* Spread to All */}
            <label className="flex items-center gap-2 text-sm text-[var(--color-text-primary)] cursor-pointer">
              <input
                type="checkbox"
                checked={spreadToAll}
                onChange={(e) => setSpreadToAll(e.target.checked)}
                className="w-4 h-4 rounded border-[var(--color-border)] text-[var(--color-accent)] focus:ring-[var(--color-accent)]"
              />
              Spread to All
            </label>

            {/* Screencast */}
            <label className="flex items-center gap-2 text-sm text-[var(--color-text-primary)] cursor-pointer">
              <input
                type="checkbox"
                checked={screencastEnabled}
                onChange={(e) => setScreencastEnabled(e.target.checked)}
                className="w-4 h-4 rounded border-[var(--color-border)] text-[var(--color-accent)] focus:ring-[var(--color-accent)]"
              />
              Screencast
            </label>

            {/* Keyboard Passthrough */}
            <label className="flex items-center gap-2 text-sm text-[var(--color-text-primary)] cursor-pointer">
              <input
                type="checkbox"
                checked={keyboardPassthrough}
                onChange={toggleKeyboardPassthrough}
                className="w-4 h-4 rounded border-[var(--color-border)] text-[var(--color-accent)] focus:ring-[var(--color-accent)]"
              />
              <Keyboard size={14} />
              Keyboard
            </label>
          </div>
        </div>
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
            <CanvasWindow
              sessionId={selectedSessionId}
              spreadToAll={spreadToAll}
            />
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
