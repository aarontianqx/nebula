import { useEffect, useState, useRef, useCallback } from "react";
import { Settings, Play, Square, Keyboard, RefreshCw, Users, MousePointer, Pipette } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useAccountStore } from "../../stores/accountStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useTauriEvents } from "../../hooks/useTauriEvents";
import ManagementDialog from "../dialogs/ManagementDialog";
import SettingsDialog from "../dialogs/SettingsDialog";
import SessionList from "../session/SessionList";
import CanvasWindow from "../canvas/CanvasWindow";
import ScriptControls from "../session/ScriptControls";

function MainWindow() {
  const { accounts, groups, fetchAccounts, fetchGroups } = useAccountStore();
  const {
    sessions,
    selectedSessionId,
    startSession,
    stopAllSessions,
    loading,
    currentFrame,
  } = useSessionStore();
  const [showManagement, setShowManagement] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [selectedAccountId, setSelectedAccountId] = useState<string>("");
  const [selectedGroupId, setSelectedGroupId] = useState<string>("");
  const [runningGroup, setRunningGroup] = useState(false);
  const [keyboardPassthrough, setKeyboardPassthrough] = useState(false);
  const [spreadToAll, setSpreadToAll] = useState(false);
  // Screencast controls streaming (true = streaming mode, false = stopped)
  // Default: false - streaming must be explicitly enabled
  const [screencastEnabled, setScreencastEnabled] = useState(false);
  // Track which session currently has screencast running (only one at a time)
  const screencastSessionRef = useRef<string | null>(null);

  // Inspector state
  const [inspectorX, setInspectorX] = useState<string>("0");
  const [inspectorY, setInspectorY] = useState<string>("0");
  const [inspectorColor, setInspectorColor] = useState<string>("");
  const [inspectorRgb, setInspectorRgb] = useState<[number, number, number]>([0, 0, 0]);

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

  const handleRunGroup = async () => {
    if (!selectedGroupId || runningGroup) return;
    setRunningGroup(true);
    try {
      await invoke("run_group", { groupId: selectedGroupId });
    } catch (error) {
      console.error("Failed to run group:", error);
    } finally {
      // Note: The group run is async in background, so we reset immediately
      // The actual sessions will appear as they are created
      setRunningGroup(false);
    }
  };

  // Check if selected account already has a session
  const hasSessionForAccount = sessions.some(
    (s) => s.account_id === selectedAccountId
  );

  // Check if selected group is empty or all accounts already have sessions
  const selectedGroup = groups.find((g) => g.id === selectedGroupId);
  const groupHasNoNewAccounts =
    selectedGroup &&
    selectedGroup.account_ids.every((accId) =>
      sessions.some((s) => s.account_id === accId)
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


  // Handle session switch: capture screenshot or clear frame
  // When switching TO a new session: don't clear old frame, let new frame overwrite it
  // When session becomes null: clear all frames to prevent stale display
  const prevSessionIdRef = useRef<string | null>(null);
  useEffect(() => {
    if (selectedSessionId !== prevSessionIdRef.current) {
      if (selectedSessionId) {
        // Switching to a new session - capture screenshot if screencast is OFF
        // Don't clear the old frame - let the new frame overwrite it naturally
        if (!screencastEnabled) {
          invoke("capture_screenshot", { sessionId: selectedSessionId }).catch(() => {
            // Ignore errors (session might not be ready yet)
          });
        }
      } else {
        // No session selected - clear the current frame
        useSessionStore.getState().clearCurrentFrame();
      }
    }
    prevSessionIdRef.current = selectedSessionId;
  }, [selectedSessionId, screencastEnabled]);

  // Manage screencast: only ONE session at a time, strictly controlled
  // When screencast is enabled: stream the selected session
  // When screencast is disabled: stop any streaming
  // When session changes while enabled: stop old, start new
  useEffect(() => {
    const manageScreencast = async () => {
      const currentScreencast = screencastSessionRef.current;
      const sessionIds = sessions.map((s) => s.id);

      // Stop screencast if the session no longer exists
      if (currentScreencast && !sessionIds.includes(currentScreencast)) {
        try {
          await invoke("stop_screencast", { sessionId: currentScreencast });
        } catch {
          // Session already gone, ignore
        }
        screencastSessionRef.current = null;
        return;
      }

      // If screencast disabled, stop any active screencast
      if (!screencastEnabled) {
        if (currentScreencast) {
          try {
            await invoke("stop_screencast", { sessionId: currentScreencast });
          } catch {
            // Ignore errors
          }
          screencastSessionRef.current = null;
        }
        return;
      }

      // Screencast enabled - need a valid selected session
      if (!selectedSessionId || !sessionIds.includes(selectedSessionId)) {
        // No valid session selected, stop any active screencast
        if (currentScreencast) {
          try {
            await invoke("stop_screencast", { sessionId: currentScreencast });
          } catch {
            // Ignore
          }
          screencastSessionRef.current = null;
        }
        return;
      }

      // If already streaming the selected session, nothing to do
      if (currentScreencast === selectedSessionId) {
        return;
      }

      // Need to switch: stop old, start new
      if (currentScreencast) {
        try {
          await invoke("stop_screencast", { sessionId: currentScreencast });
        } catch {
          // Ignore
        }
      }

      try {
        await invoke("start_screencast", { sessionId: selectedSessionId });
        screencastSessionRef.current = selectedSessionId;
      } catch (error) {
        console.error("Failed to start screencast:", error);
        screencastSessionRef.current = null;
      }
    };

    manageScreencast();
  }, [screencastEnabled, selectedSessionId, sessions]);

  const handleRefresh = async () => {
    if (!selectedSessionId) return;
    try {
      await invoke("refresh_session", { sessionId: selectedSessionId });
    } catch (error) {
      console.error("Failed to refresh:", error);
    }
  };

  // Inspector: fetch color at X/Y from current frame
  const fetchColorAtCoordinates = useCallback(() => {
    if (!selectedSessionId || !currentFrame) return;

    const x = parseInt(inspectorX) || 0;
    const y = parseInt(inspectorY) || 0;

    // Decode base64 frame and extract color
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = img.width;
      canvas.height = img.height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(img, 0, 0);

      // Clamp coordinates
      const clampedX = Math.max(0, Math.min(x, img.width - 1));
      const clampedY = Math.max(0, Math.min(y, img.height - 1));

      const pixel = ctx.getImageData(clampedX, clampedY, 1, 1).data;
      const [r, g, b] = [pixel[0], pixel[1], pixel[2]];
      setInspectorRgb([r, g, b]);
      setInspectorColor(`RGB(${r}, ${g}, ${b})`);
    };
    img.src = `data:image/jpeg;base64,${currentFrame}`;
  }, [selectedSessionId, currentFrame, inspectorX, inspectorY]);

  // Update inspector when coordinates change via keyboard input
  const handleInspectorKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      fetchColorAtCoordinates();
    }
  };

  // Click button: execute click at X/Y coordinates
  const handleClickAtCoordinates = async () => {
    if (!selectedSessionId) return;
    const x = parseInt(inspectorX) || 0;
    const y = parseInt(inspectorY) || 0;

    try {
      if (spreadToAll) {
        await invoke("click_all_sessions", { x, y });
      } else {
        await invoke("click_session", { sessionId: selectedSessionId, x, y });
      }
    } catch (error) {
      console.error("Failed to click:", error);
    }
  };

  // Handle canvas click: update inspector and optionally forward to browser
  const handleCanvasClick = useCallback(
    (x: number, y: number) => {
      // Always update inspector coordinates and color
      setInspectorX(Math.round(x).toString());
      setInspectorY(Math.round(y).toString());

      // Fetch color at clicked position
      if (selectedSessionId && currentFrame) {
        const img = new Image();
        img.onload = () => {
          const canvas = document.createElement("canvas");
          canvas.width = img.width;
          canvas.height = img.height;
          const ctx = canvas.getContext("2d");
          if (!ctx) return;
          ctx.drawImage(img, 0, 0);

          const clampedX = Math.max(0, Math.min(Math.round(x), img.width - 1));
          const clampedY = Math.max(0, Math.min(Math.round(y), img.height - 1));

          const pixel = ctx.getImageData(clampedX, clampedY, 1, 1).data;
          const [r, g, b] = [pixel[0], pixel[1], pixel[2]];
          setInspectorRgb([r, g, b]);
          setInspectorColor(`RGB(${r}, ${g}, ${b})`);
        };
        img.src = `data:image/jpeg;base64,${currentFrame}`;
      }
    },
    [selectedSessionId, currentFrame]
  );

  // Handle canvas mouse up: forward click to browser only if screencast enabled
  const handleCanvasMouseAction = useCallback(
    async (action: "click" | "drag", x: number, y: number, endX?: number, endY?: number) => {
      if (!selectedSessionId) return;

      if (screencastEnabled) {
        // Screencast ON: forward mouse actions to browser
        if (action === "click") {
          if (spreadToAll) {
            await invoke("click_all_sessions", { x, y });
          } else {
            await invoke("click_session", { sessionId: selectedSessionId, x, y });
          }
        } else if (action === "drag" && endX !== undefined && endY !== undefined) {
          if (spreadToAll) {
            await invoke("drag_all_sessions", { fromX: x, fromY: y, toX: endX, toY: endY });
          } else {
            await invoke("drag_session", {
              sessionId: selectedSessionId,
              fromX: x,
              fromY: y,
              toX: endX,
              toY: endY,
            });
          }
        }
      } else {
        // Screencast OFF: capture a single screenshot to update the canvas
        // This does NOT reload the browser page, just takes a screenshot
        try {
          await invoke("capture_screenshot", { sessionId: selectedSessionId });
        } catch (error) {
          console.error("Failed to capture screenshot:", error);
        }
      }
    },
    [selectedSessionId, screencastEnabled, spreadToAll]
  );

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
              title="Run selected account"
            >
              <Play size={14} />
              Run
            </button>

            {/* Divider */}
            <div className="w-px h-5 bg-[var(--color-border)]" />

            {/* Group selector */}
            <select
              value={selectedGroupId}
              onChange={(e) => setSelectedGroupId(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] border border-[var(--color-border)] focus:outline-none focus:border-[var(--color-accent)] min-w-[140px]"
            >
              <option value="">Select Group</option>
              {groups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </select>

            {/* Run Group button */}
            <button
              onClick={handleRunGroup}
              disabled={
                !selectedGroupId ||
                runningGroup ||
                groupHasNoNewAccounts ||
                loading
              }
              className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-success)] text-white hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
              title="Run all accounts in the selected group"
            >
              <Users size={14} />
              Run Group
            </button>

            {/* Divider */}
            <div className="w-px h-5 bg-[var(--color-border)]" />

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
            <Users size={16} />
            Manage
          </button>
          <button
            onClick={() => setShowSettings(true)}
            className="flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] hover:bg-[var(--color-border)] transition-colors"
          >
            <Settings size={16} />
            Settings
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
        <div className="flex-1 flex flex-col items-center justify-center p-4 overflow-auto">
          {selectedSessionId ? (
            <>
              <CanvasWindow
                sessionId={selectedSessionId}
                onCanvasClick={handleCanvasClick}
                onMouseAction={handleCanvasMouseAction}
                keyboardPassthrough={keyboardPassthrough}
                spreadToAll={spreadToAll}
              />
              {/* Inspector Panel - HUD style below canvas */}
              <div className="mt-4 flex items-center gap-4 px-5 py-3 bg-[var(--color-bg-panel)] rounded-lg border border-[var(--color-border)] shadow-sm">
                {/* Coordinates Section */}
                <div className="flex items-center gap-3">
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-medium text-[var(--color-text-muted)] uppercase tracking-wide">X</span>
                    <input
                      type="number"
                      value={inspectorX}
                      onChange={(e) => setInspectorX(e.target.value)}
                      onKeyDown={handleInspectorKeyDown}
                      className="w-16 px-2 py-1.5 text-sm font-mono bg-[var(--color-bg-surface)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)] focus:ring-1 focus:ring-[var(--color-accent)]/20"
                    />
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs font-medium text-[var(--color-text-muted)] uppercase tracking-wide">Y</span>
                    <input
                      type="number"
                      value={inspectorY}
                      onChange={(e) => setInspectorY(e.target.value)}
                      onKeyDown={handleInspectorKeyDown}
                      className="w-16 px-2 py-1.5 text-sm font-mono bg-[var(--color-bg-surface)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)] focus:ring-1 focus:ring-[var(--color-accent)]/20"
                    />
                  </div>
                </div>

                {/* Divider */}
                <div className="w-px h-7 bg-[var(--color-border)]" />

                {/* Action Buttons */}
                <div className="flex items-center gap-2">
                  <button
                    onClick={fetchColorAtCoordinates}
                    disabled={!selectedSessionId}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-surface)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] disabled:opacity-50 disabled:cursor-not-allowed transition-colors border border-[var(--color-border)]"
                    title="Fetch color at coordinates (Enter)"
                  >
                    <Pipette size={14} />
                    Fetch
                  </button>

                  <button
                    onClick={handleClickAtCoordinates}
                    disabled={!selectedSessionId}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md bg-[var(--color-bg-surface)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] disabled:opacity-50 disabled:cursor-not-allowed transition-colors border border-[var(--color-border)]"
                    title="Click at coordinates (respects Spread to All)"
                  >
                    <MousePointer size={14} />
                    Click
                  </button>
                </div>

                {/* Divider */}
                <div className="w-px h-7 bg-[var(--color-border)]" />

                {/* Color Preview Section */}
                <div className="flex items-center gap-3">
                  <div
                    className="w-8 h-8 rounded-md border-2 border-[var(--color-border)] shadow-inner"
                    style={{
                      backgroundColor: `rgb(${inspectorRgb[0]}, ${inspectorRgb[1]}, ${inspectorRgb[2]})`,
                    }}
                    title={inspectorColor}
                  />
                  <span className="text-sm text-[var(--color-text-secondary)] font-mono tabular-nums">
                    {inspectorColor || "RGB(0, 0, 0)"}
                  </span>
                </div>
              </div>
            </>
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

      {/* Settings Dialog */}
      {showSettings && (
        <SettingsDialog
          onClose={() => setShowSettings(false)}
          onThemeChange={() => {
            // Theme changes require app restart, so just close for now
          }}
        />
      )}
    </div>
  );
}

export default MainWindow;
