import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import React from "react";

type EngineState = "Idle" | "Arming" | "Running" | "Paused" | "Stopped";

type ActionInfo =
  | { Click: { x: number; y: number; button: string } }
  | { KeyTap: { key: string } }
  | { Wait: { ms: number } }
  | { [key: string]: unknown };

type EngineEvent =
  | { StateChanged: { old: EngineState; new: EngineState } }
  | { CountdownTick: { remaining_secs: number } }
  | { ActionStarting: { index: number; action: ActionInfo } }
  | { ActionCompleted: { index: number } }
  | { IterationCompleted: { iteration: number } }
  | "Completed"
  | { Error: { message: string } };

function formatAction(action: ActionInfo): string {
  if ("Click" in action) {
    return `Click @ (${action.Click.x}, ${action.Click.y})`;
  } else if ("KeyTap" in action) {
    return `Key "${action.KeyTap.key}"`;
  } else if ("Wait" in action) {
    return `Wait ${action.Wait.ms}ms`;
  }
  return JSON.stringify(action);
}

function formatTime(): string {
  const now = new Date();
  const h = now.getHours().toString().padStart(2, "0");
  const m = now.getMinutes().toString().padStart(2, "0");
  const s = now.getSeconds().toString().padStart(2, "0");
  const ms = now.getMilliseconds().toString().padStart(3, "0");
  return `${h}:${m}:${s}.${ms}`;
}

interface LogEntry {
  time: string;
  message: string;
}

export default function App() {
  // Engine state (always shown)
  const [engineState, setEngineState] = React.useState<EngineState>("Idle");
  const [countdown, setCountdown] = React.useState<number | null>(null);
  const [executedCount, setExecutedCount] = React.useState<number>(0);
  const [iteration, setIteration] = React.useState<number>(0);
  const [lastAction, setLastAction] = React.useState<string | null>(null);

  // Separate status messages
  const [engineStatus, setEngineStatus] = React.useState<string>("Ready");
  const [uiMessage, setUiMessage] = React.useState<string | null>(null);

  // Use ref for logs to avoid closure issues
  const [logs, setLogs] = React.useState<LogEntry[]>([]);
  const logsRef = React.useRef<LogEntry[]>([]);

  // Config state
  const [actionType, setActionType] = React.useState<"click" | "key">("click");
  const [clickX, setClickX] = React.useState<number>(640);
  const [clickY, setClickY] = React.useState<number>(360);
  const [keyName, setKeyName] = React.useState<string>("Space");
  const [intervalMs, setIntervalMs] = React.useState<number>(1000);
  const [repeatCount, setRepeatCount] = React.useState<string>("");
  const [countdownSecs, setCountdownSecs] = React.useState<number>(3);

  // Position picker state
  const [isPicking, setIsPicking] = React.useState<boolean>(false);
  const [mousePos, setMousePos] = React.useState<{ x: number; y: number } | null>(null);

  const logContainerRef = React.useRef<HTMLDivElement>(null);

  const addLog = React.useCallback((msg: string) => {
    const entry: LogEntry = { time: formatTime(), message: msg };
    logsRef.current = [...logsRef.current, entry].slice(-100);
    setLogs([...logsRef.current]);
  }, []);

  // Auto-scroll to bottom when logs change
  React.useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs]);

  // Clear UI message after delay
  React.useEffect(() => {
    if (uiMessage) {
      const timer = setTimeout(() => setUiMessage(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [uiMessage]);

  // Listen to backend events
  React.useEffect(() => {
    let unlistenEngine: UnlistenFn | null = null;
    let unlistenEmergency: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenEngine = await listen<EngineEvent>("engine-event", (event) => {
        const e = event.payload;
        console.log("Engine event:", e);

        if ("StateChanged" in e) {
          setEngineState(e.StateChanged.new);
          if (e.StateChanged.new === "Idle") {
            setCountdown(null);
            setLastAction(null);
            setEngineStatus("Ready");
          } else if (e.StateChanged.new === "Running") {
            setEngineStatus("Running");
          } else if (e.StateChanged.new === "Paused") {
            setEngineStatus("Paused");
          } else if (e.StateChanged.new === "Stopped") {
            setEngineStatus("Stopped");
          } else if (e.StateChanged.new === "Arming") {
            setEngineStatus("Arming...");
          }
          addLog(`State: ${e.StateChanged.old} → ${e.StateChanged.new}`);
        } else if ("CountdownTick" in e) {
          setCountdown(e.CountdownTick.remaining_secs);
          setEngineStatus(`Starting in ${e.CountdownTick.remaining_secs}...`);
        } else if ("ActionStarting" in e) {
          const actionStr = formatAction(e.ActionStarting.action);
          setLastAction(actionStr);
          setEngineStatus(`Executing: ${actionStr}`);
          addLog(`→ ${actionStr}`);
        } else if ("ActionCompleted" in e) {
          setExecutedCount((c) => c + 1);
        } else if ("IterationCompleted" in e) {
          setIteration(e.IterationCompleted.iteration);
          addLog(`✓ Iter #${e.IterationCompleted.iteration}`);
        } else if (e === "Completed") {
          setEngineStatus("✅ Completed!");
          addLog("✓ All done");
        } else if ("Error" in e) {
          setEngineStatus(`❌ ${e.Error.message}`);
          addLog(`❌ ${e.Error.message}`);
        }
      });

      unlistenEmergency = await listen("emergency-stop", () => {
        setEngineStatus("⚠️ Emergency stopped!");
        addLog("⚠️ EMERGENCY STOP");
      });
    };

    setupListeners();

    return () => {
      if (unlistenEngine) unlistenEngine();
      if (unlistenEmergency) unlistenEmergency();
    };
  }, [addLog]);

  // Mouse position tracking
  React.useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      setMousePos({ x: e.screenX, y: e.screenY });
    };

    const handleClick = (e: MouseEvent) => {
      if (isPicking) {
        setClickX(e.screenX);
        setClickY(e.screenY);
        setIsPicking(false);
        setUiMessage(`Picked: (${e.screenX}, ${e.screenY})`);
        addLog(`📍 Picked: (${e.screenX}, ${e.screenY})`);
        e.preventDefault();
        e.stopPropagation();
      }
    };

    if (isPicking) {
      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("click", handleClick, true);
      return () => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("click", handleClick, true);
      };
    } else {
      document.addEventListener("mousemove", handleMouseMove);
      return () => {
        document.removeEventListener("mousemove", handleMouseMove);
      };
    }
  }, [isPicking, addLog]);

  async function handleStart() {
    try {
      await invoke("set_simple_repeat", {
        actionType,
        x: actionType === "click" ? clickX : null,
        y: actionType === "click" ? clickY : null,
        key: actionType === "key" ? keyName : null,
        intervalMs,
        repeatCount: repeatCount ? parseInt(repeatCount, 10) : null,
        countdownSecs,
      });

      setExecutedCount(0);
      setIteration(0);
      logsRef.current = [];
      setLogs([]);

      await invoke("start_execution");
      setEngineStatus("Starting...");
      addLog("▶ Started");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
      addLog(`❌ ${String(e)}`);
    }
  }

  async function handlePause() {
    try {
      await invoke("pause_execution");
      addLog("⏸ Paused");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
    }
  }

  async function handleResume() {
    try {
      await invoke("resume_execution");
      addLog("▶ Resumed");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
    }
  }

  async function handleStop() {
    try {
      await invoke("stop_execution");
      addLog("⏹ Stopped");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
    }
  }

  async function handleEmergencyStop() {
    try {
      await invoke("emergency_stop");
      addLog("⚠️ EMERGENCY STOP");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
    }
  }

  function handlePickPosition() {
    setIsPicking(true);
    setUiMessage("🎯 Click anywhere to pick...");
  }

  function handleCancelPick() {
    setIsPicking(false);
    setUiMessage(null);
  }

  const isIdle = engineState === "Idle";
  const isRunning = engineState === "Running";
  const isPaused = engineState === "Paused";
  const isArming = engineState === "Arming";

  const displayStatus = uiMessage || engineStatus;

  return (
    <div className={`app ${isPicking ? "picking-mode" : ""}`}>
      <header className="topbar">
        <div className="brand">
          <div className="logo">tap</div>
          <div className="subtitle">Timed Action Performer</div>
        </div>
        <div className="topbar-actions">
          {mousePos && (
            <span className="mouse-pos">
              🖱️ ({mousePos.x}, {mousePos.y})
            </span>
          )}
          <span className={`state-badge state-${engineState.toLowerCase()}`}>
            {engineState}
          </span>
        </div>
      </header>

      <div className="layout">
        <aside className="sidebar">
          <h3>Configuration</h3>
          <div className="card">
            <div className="field">
              <label className="label">Action</label>
              <select
                value={actionType}
                onChange={(e) => setActionType(e.target.value as "click" | "key")}
                disabled={!isIdle}
                className="input"
              >
                <option value="click">Click</option>
                <option value="key">Key Press</option>
              </select>
            </div>

            {actionType === "click" && (
              <>
                <div className="field">
                  <label className="label">X</label>
                  <div className="input-with-button">
                    <input
                      type="number"
                      value={clickX}
                      onChange={(e) => setClickX(parseInt(e.target.value, 10) || 0)}
                      disabled={!isIdle || isPicking}
                      className="input"
                    />
                  </div>
                </div>
                <div className="field">
                  <label className="label">Y</label>
                  <div className="input-with-button">
                    <input
                      type="number"
                      value={clickY}
                      onChange={(e) => setClickY(parseInt(e.target.value, 10) || 0)}
                      disabled={!isIdle || isPicking}
                      className="input"
                    />
                    {isPicking ? (
                      <button
                        className="btn btn-pick picking"
                        onClick={handleCancelPick}
                        title="Cancel picking"
                      >
                        ✕ Cancel
                      </button>
                    ) : (
                      <button
                        className="btn btn-pick"
                        onClick={handlePickPosition}
                        disabled={!isIdle}
                        title="Click to pick position from screen"
                      >
                        🎯 Pick
                      </button>
                    )}
                  </div>
                </div>
              </>
            )}

            {actionType === "key" && (
              <div className="field">
                <label className="label">Key</label>
                <input
                  type="text"
                  value={keyName}
                  onChange={(e) => setKeyName(e.target.value)}
                  disabled={!isIdle}
                  className="input"
                  placeholder="e.g., Space, Enter, a"
                />
              </div>
            )}

            <div className="field">
              <label className="label">Interval</label>
              <div className="input-suffix">
                <input
                  type="number"
                  value={intervalMs}
                  onChange={(e) => setIntervalMs(parseInt(e.target.value, 10) || 100)}
                  disabled={!isIdle}
                  className="input"
                  min={50}
                />
                <span>ms</span>
              </div>
            </div>

            <div className="field">
              <label className="label">Repeat</label>
              <input
                type="text"
                value={repeatCount}
                onChange={(e) => setRepeatCount(e.target.value)}
                disabled={!isIdle}
                className="input"
                placeholder="∞ (empty = forever)"
              />
            </div>

            <div className="field">
              <label className="label">Countdown</label>
              <div className="input-suffix">
                <input
                  type="number"
                  value={countdownSecs}
                  onChange={(e) => setCountdownSecs(parseInt(e.target.value, 10) || 0)}
                  disabled={!isIdle}
                  className="input"
                  min={0}
                />
                <span>sec</span>
              </div>
            </div>
          </div>

          <h3>Safety</h3>
          <div className="card safety-card">
            <div className="safety-info">
              <span className="safety-icon">⌨️</span>
              <div>
                <div className="safety-title">Emergency Stop</div>
                <div className="safety-key">Ctrl + Shift + Backspace</div>
              </div>
            </div>
          </div>
        </aside>

        <main className="main">
          <h3>Controls</h3>
          <div className="card controls-card">
            <div className="control-buttons">
              {isIdle && (
                <button className="btn btn-primary btn-large" onClick={handleStart}>
                  ▶ Start
                </button>
              )}
              {isArming && (
                <div className="countdown-display">
                  <span className="countdown-number">{countdown}</span>
                  <span className="countdown-label">Starting in...</span>
                </div>
              )}
              {isRunning && (
                <>
                  <button className="btn" onClick={handlePause}>
                    ⏸ Pause
                  </button>
                  <button className="btn btn-danger" onClick={handleStop}>
                    ⏹ Stop
                  </button>
                </>
              )}
              {isPaused && (
                <>
                  <button className="btn btn-primary" onClick={handleResume}>
                    ▶ Resume
                  </button>
                  <button className="btn btn-danger" onClick={handleStop}>
                    ⏹ Stop
                  </button>
                </>
              )}
            </div>

            {(isRunning || isPaused || isArming) && (
              <div className="stats">
                <div className="stat">
                  <span className="stat-value">{executedCount}</span>
                  <span className="stat-label">Actions</span>
                </div>
                <div className="stat">
                  <span className="stat-value">{iteration}</span>
                  <span className="stat-label">Iterations</span>
                </div>
              </div>
            )}

            {lastAction && (isRunning || isPaused) && (
              <div className="last-action">
                <span className="last-action-label">Last:</span>
                <span className="last-action-value">{lastAction}</span>
              </div>
            )}

            <button
              className="btn btn-emergency"
              onClick={handleEmergencyStop}
              disabled={isIdle}
            >
              ⚠️ Emergency Stop
            </button>
          </div>

          <h3>Activity Log</h3>
          <div className="card log-card">
            <div className="log-container" ref={logContainerRef}>
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
        </main>
      </div>

      <footer className="statusbar">
        <span className={`status-state state-${engineState.toLowerCase()}`}>
          {engineState}
        </span>
        <span className="status-divider">|</span>
        <span className="status-value">{displayStatus}</span>
        {(isRunning || isPaused) && (
          <>
            <span className="status-divider">|</span>
            <span className="status-stats">
              {executedCount} actions, {iteration} iters
            </span>
          </>
        )}
      </footer>

      {isPicking && <div className="picking-overlay" onClick={handleCancelPick} />}
    </div>
  );
}
