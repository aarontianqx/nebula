import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import React from "react";

// Types
type EngineState = "Idle" | "Arming" | "Running" | "Paused" | "Stopped";
type RecorderState = "Idle" | "Recording" | "Paused";
type Mode = "simple" | "timeline";

interface TimedAction {
  at_ms: number;
  action: ActionInfo;
  enabled: boolean;
  note: string | null;
}

interface Timeline {
  actions: TimedAction[];
}

interface Profile {
  name: string;
  timeline: Timeline;
  run: {
    start_delay_ms: number;
    speed: number;
    repeat: { Times: number } | "Forever";
  };
}

type ActionInfo =
  | { Click: { x: number; y: number; button: string } }
  | { DoubleClick: { x: number; y: number; button: string } }
  | { MouseDown: { x: number; y: number; button: string } }
  | { MouseUp: { x: number; y: number; button: string } }
  | { MouseMove: { x: number; y: number } }
  | { Drag: { from: { x: number; y: number }; to: { x: number; y: number }; duration_ms: number } }
  | { KeyTap: { key: string } }
  | { KeyDown: { key: string } }
  | { KeyUp: { key: string } }
  | { TextInput: { text: string } }
  | { Wait: { ms: number } }
  | { Scroll: { delta_x: number; delta_y: number } };

type EngineEvent =
  | { StateChanged: { old: EngineState; new: EngineState } }
  | { CountdownTick: { remaining_secs: number } }
  | { ActionStarting: { index: number; action: ActionInfo } }
  | { ActionCompleted: { index: number } }
  | { IterationCompleted: { iteration: number } }
  | "Completed"
  | { Error: { message: string } };

interface RecordingStatus {
  state: RecorderState;
  event_count: number;
  duration_ms: number;
}

interface LogEntry {
  time: string;
  message: string;
}

// Helpers
function formatAction(action: ActionInfo): string {
  if ("Click" in action) return `Click @ (${action.Click.x}, ${action.Click.y})`;
  if ("DoubleClick" in action) return `DblClick @ (${action.DoubleClick.x}, ${action.DoubleClick.y})`;
  if ("MouseDown" in action) return `MouseDown @ (${action.MouseDown.x}, ${action.MouseDown.y})`;
  if ("MouseUp" in action) return `MouseUp @ (${action.MouseUp.x}, ${action.MouseUp.y})`;
  if ("MouseMove" in action) return `Move → (${action.MouseMove.x}, ${action.MouseMove.y})`;
  if ("Drag" in action) return `Drag (${action.Drag.from.x},${action.Drag.from.y}) → (${action.Drag.to.x},${action.Drag.to.y})`;
  if ("KeyTap" in action) return `Key "${action.KeyTap.key}"`;
  if ("KeyDown" in action) return `KeyDown "${action.KeyDown.key}"`;
  if ("KeyUp" in action) return `KeyUp "${action.KeyUp.key}"`;
  if ("TextInput" in action) return `Type "${action.TextInput.text}"`;
  if ("Wait" in action) return `Wait ${action.Wait.ms}ms`;
  if ("Scroll" in action) return `Scroll (${action.Scroll.delta_x}, ${action.Scroll.delta_y})`;
  // Exhaustive check - should never reach here if all types are handled
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

function formatDuration(ms: number): string {
  const secs = Math.floor(ms / 1000);
  const mins = Math.floor(secs / 60);
  const remSecs = secs % 60;
  const remMs = ms % 1000;
  if (mins > 0) {
    return `${mins}:${remSecs.toString().padStart(2, "0")}.${Math.floor(remMs / 100)}`;
  }
  return `${secs}.${Math.floor(remMs / 100)}s`;
}

export default function App() {
  // Mode
  const [mode, setMode] = React.useState<Mode>("simple");

  // Engine state
  const [engineState, setEngineState] = React.useState<EngineState>("Idle");
  const [countdown, setCountdown] = React.useState<number | null>(null);
  const [executedCount, setExecutedCount] = React.useState<number>(0);
  const [iteration, setIteration] = React.useState<number>(0);
  const [lastAction, setLastAction] = React.useState<string | null>(null);

  // Recording state
  const [recorderState, setRecorderState] = React.useState<RecorderState>("Idle");
  const [recordingEventCount, setRecordingEventCount] = React.useState<number>(0);
  const [recordingDuration, setRecordingDuration] = React.useState<number>(0);

  // Timeline state
  const [timeline, setTimeline] = React.useState<TimedAction[]>([]);
  const [selectedActionIdx, setSelectedActionIdx] = React.useState<number | null>(null);

  // Playback config
  const [speed, setSpeed] = React.useState<number>(1.0);
  const [repeatCount, setRepeatCount] = React.useState<string>("");
  const [countdownSecs, setCountdownSecs] = React.useState<number>(3);

  // Simple mode config
  const [actionType, setActionType] = React.useState<"click" | "key">("click");
  const [clickX, setClickX] = React.useState<number>(640);
  const [clickY, setClickY] = React.useState<number>(360);
  const [keyName, setKeyName] = React.useState<string>("Space");
  const [intervalMs, setIntervalMs] = React.useState<number>(1000);

  // Profile state
  const [profileName, setProfileName] = React.useState<string>("Untitled");
  const [profiles, setProfiles] = React.useState<string[]>([]);

  // UI state
  const [engineStatus, setEngineStatus] = React.useState<string>("Ready");
  const [uiMessage, setUiMessage] = React.useState<string | null>(null);
  const [logs, setLogs] = React.useState<LogEntry[]>([]);
  const logsRef = React.useRef<LogEntry[]>([]);
  const [mousePos, setMousePos] = React.useState<{ x: number; y: number } | null>(null);
  const logContainerRef = React.useRef<HTMLDivElement>(null);

  const addLog = React.useCallback((msg: string) => {
    const entry: LogEntry = { time: formatTime(), message: msg };
    logsRef.current = [...logsRef.current, entry].slice(-100);
    setLogs([...logsRef.current]);
  }, []);

  // Load profiles on mount
  React.useEffect(() => {
    invoke<string[]>("cmd_list_profiles").then(setProfiles).catch(console.error);
  }, []);

  // Auto-scroll logs
  React.useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs]);

  // Clear UI message
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
    let unlistenRecording: UnlistenFn | null = null;
    let unlistenMousePos: UnlistenFn | null = null;
    let unlistenPositionPicked: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenEngine = await listen<EngineEvent>("engine-event", (event) => {
        const e = event.payload;

        // Handle string literal "Completed" first to narrow the type
        if (e === "Completed") {
          setEngineStatus("✅ Completed!");
          addLog("✓ All done");
          return;
        }

        // Now e is narrowed to object types only
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
        } else if ("Error" in e) {
          setEngineStatus(`❌ ${e.Error.message}`);
          addLog(`❌ ${e.Error.message}`);
        }
      });

      unlistenEmergency = await listen("emergency-stop", () => {
        setEngineStatus("⚠️ Emergency stopped!");
        addLog("⚠️ EMERGENCY STOP");
      });

      unlistenRecording = await listen<RecordingStatus>("recording-status", (event) => {
        const s = event.payload;
        setRecorderState(s.state);
        setRecordingEventCount(s.event_count);
        setRecordingDuration(s.duration_ms);
      });

      // Listen for global mouse position updates from backend (via rdev)
      unlistenMousePos = await listen<{ x: number; y: number }>("mouse-position", (event) => {
        setMousePos(event.payload);
      });

      // Listen for position picked events (global click while in pick mode)
      unlistenPositionPicked = await listen<{ x: number; y: number }>("position-picked", (event) => {
        const { x, y } = event.payload;
        setClickX(x);
        setClickY(y);
        setUiMessage(`Picked: (${x}, ${y})`);
        addLog(`📍 Picked: (${x}, ${y})`);
      });
    };

    setupListeners();
    return () => {
      unlistenEngine?.();
      unlistenEmergency?.();
      unlistenRecording?.();
      unlistenMousePos?.();
      unlistenPositionPicked?.();
    };
  }, [addLog]);

  // Handlers
  async function handleStartSimple() {
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
      addLog("▶ Started");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
      addLog(`❌ ${String(e)}`);
    }
  }

  async function handleStartTimeline() {
    try {
      setExecutedCount(0);
      setIteration(0);
      logsRef.current = [];
      setLogs([]);
      await invoke("start_execution");
      addLog("▶ Playing timeline");
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

  // Recording handlers
  async function handleStartRecording() {
    try {
      await invoke("start_recording");
      setRecorderState("Recording");
      setRecordingEventCount(0);
      setRecordingDuration(0);
      addLog("🔴 Recording started");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
      addLog(`❌ ${String(e)}`);
    }
  }

  async function handlePauseRecording() {
    try {
      await invoke("pause_recording");
      addLog("⏸ Recording paused");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
    }
  }

  async function handleResumeRecording() {
    try {
      await invoke("resume_recording");
      addLog("🔴 Recording resumed");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
    }
  }

  async function handleStopRecording() {
    try {
      const result = await invoke<Timeline>("stop_recording");
      setTimeline(result.actions);
      setRecorderState("Idle");
      addLog(`⏹ Recording stopped: ${result.actions.length} actions`);
      setMode("timeline");
    } catch (e) {
      setEngineStatus(`Failed: ${String(e)}`);
      addLog(`❌ ${String(e)}`);
    }
  }

  // Profile handlers
  async function handleSaveProfile() {
    try {
      await invoke("cmd_save_profile", { name: profileName });
      const list = await invoke<string[]>("cmd_list_profiles");
      setProfiles(list);
      addLog(`💾 Saved: ${profileName}`);
    } catch (e) {
      addLog(`❌ ${String(e)}`);
    }
  }

  async function handleLoadProfile(name: string) {
    try {
      const profile = await invoke<Profile>("cmd_load_profile", { name });
      setProfileName(profile.name);
      setTimeline(profile.timeline.actions);
      setSpeed(profile.run.speed);
      if (profile.run.repeat === "Forever") {
        setRepeatCount("");
      } else {
        setRepeatCount(String(profile.run.repeat.Times));
      }
      setCountdownSecs(Math.floor(profile.run.start_delay_ms / 1000));
      setMode("timeline");
      addLog(`📂 Loaded: ${name}`);
    } catch (e) {
      addLog(`❌ ${String(e)}`);
    }
  }

  // Timeline editing
  function handleToggleAction(idx: number) {
    setTimeline((prev) =>
      prev.map((a, i) => (i === idx ? { ...a, enabled: !a.enabled } : a))
    );
  }

  function handleDeleteAction(idx: number) {
    setTimeline((prev) => prev.filter((_, i) => i !== idx));
    setSelectedActionIdx(null);
  }

  function handleAdjustDelay(idx: number, delta: number) {
    setTimeline((prev) =>
      prev.map((a, i) =>
        i === idx ? { ...a, at_ms: Math.max(0, a.at_ms + delta) } : a
      )
    );
  }

  const isIdle = engineState === "Idle";
  const isRunning = engineState === "Running";
  const isPaused = engineState === "Paused";
  const isArming = engineState === "Arming";
  const isRecording = recorderState === "Recording";
  const isRecordingPaused = recorderState === "Paused";
  const canRecord = isIdle && recorderState === "Idle";

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <div className="logo">tap</div>
          <div className="subtitle">Timed Action Performer</div>
        </div>
        <div className="topbar-tabs">
          <button
            className={`tab ${mode === "simple" ? "active" : ""}`}
            onClick={() => setMode("simple")}
            disabled={!isIdle || isRecording}
          >
            Simple
          </button>
          <button
            className={`tab ${mode === "timeline" ? "active" : ""}`}
            onClick={() => setMode("timeline")}
            disabled={!isIdle || isRecording}
          >
            Timeline
          </button>
        </div>
        <div className="topbar-actions">
          {mousePos && (
            <span className="mouse-pos">🖱️ ({mousePos.x}, {mousePos.y})</span>
          )}
          {isRecording && (
            <span className="recording-badge">
              🔴 {formatDuration(recordingDuration)} | {recordingEventCount} events
            </span>
          )}
          <span className={`state-badge state-${engineState.toLowerCase()}`}>
            {engineState}
          </span>
        </div>
      </header>

      <div className="layout">
        <aside className="sidebar">
          {mode === "simple" ? (
            <>
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
                      <input
                        type="number"
                        value={clickX}
                        onChange={(e) => setClickX(parseInt(e.target.value, 10) || 0)}
                        disabled={!isIdle}
                        className="input"
                      />
                    </div>
                    <div className="field">
                      <label className="label">Y</label>
                      <div className="input-with-button">
                        <input
                          type="number"
                          value={clickY}
                          onChange={(e) => setClickY(parseInt(e.target.value, 10) || 0)}
                          disabled={!isIdle}
                          className="input"
                        />
                        <button
                          className="btn btn-pick"
                          onClick={async () => {
                            // Open picker window
                            await invoke("open_picker_window").catch(console.error);
                          }}
                          disabled={!isIdle}
                        >
                          🎯 Pick
                        </button>
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
                      placeholder="e.g., Space, Enter"
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
            </>
          ) : (
            <>
              <h3>Profiles</h3>
              <div className="card">
                <div className="field">
                  <label className="label">Name</label>
                  <input
                    type="text"
                    value={profileName}
                    onChange={(e) => setProfileName(e.target.value)}
                    className="input"
                  />
                </div>
                <button className="btn btn-block" onClick={handleSaveProfile} disabled={!isIdle}>
                  💾 Save
                </button>
                {profiles.length > 0 && (
                  <div className="profile-list">
                    {profiles.map((p) => (
                      <button
                        key={p}
                        className={`profile-item ${p === profileName ? "active" : ""}`}
                        onClick={() => handleLoadProfile(p)}
                        disabled={!isIdle}
                      >
                        {p}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              <h3>Playback</h3>
              <div className="card">
                <div className="field">
                  <label className="label">Speed</label>
                  <select
                    value={speed}
                    onChange={(e) => setSpeed(parseFloat(e.target.value))}
                    disabled={!isIdle}
                    className="input"
                  >
                    <option value="0.5">0.5x</option>
                    <option value="1">1x</option>
                    <option value="2">2x</option>
                    <option value="4">4x</option>
                  </select>
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
            </>
          )}

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
              {mode === "timeline" && canRecord && (
                <button className="btn btn-record" onClick={handleStartRecording}>
                  🔴 Record
                </button>
              )}
              {isRecording && (
                <>
                  <button className="btn" onClick={handlePauseRecording}>⏸ Pause</button>
                  <button className="btn btn-danger" onClick={handleStopRecording}>⏹ Stop</button>
                </>
              )}
              {isRecordingPaused && (
                <>
                  <button className="btn btn-record" onClick={handleResumeRecording}>🔴 Resume</button>
                  <button className="btn btn-danger" onClick={handleStopRecording}>⏹ Stop</button>
                </>
              )}
              {recorderState === "Idle" && isIdle && (
                <button
                  className="btn btn-primary btn-large"
                  onClick={mode === "simple" ? handleStartSimple : handleStartTimeline}
                  disabled={mode === "timeline" && timeline.length === 0}
                >
                  ▶ Play
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
                  <button className="btn" onClick={handlePause}>⏸ Pause</button>
                  <button className="btn btn-danger" onClick={handleStop}>⏹ Stop</button>
                </>
              )}
              {isPaused && (
                <>
                  <button className="btn btn-primary" onClick={handleResume}>▶ Resume</button>
                  <button className="btn btn-danger" onClick={handleStop}>⏹ Stop</button>
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

            <button className="btn btn-emergency" onClick={handleEmergencyStop} disabled={isIdle && recorderState === "Idle"}>
              ⚠️ Emergency Stop
            </button>
          </div>

          {mode === "timeline" && (
            <>
              <h3>Timeline ({timeline.length} actions)</h3>
              <div className="card timeline-card">
                {timeline.length === 0 ? (
                  <div className="timeline-empty">
                    No actions yet. Click "Record" to capture actions.
                  </div>
                ) : (
                  <div className="timeline-list">
                    {timeline.map((action, idx) => (
                      <div
                        key={idx}
                        className={`timeline-item ${!action.enabled ? "disabled" : ""} ${selectedActionIdx === idx ? "selected" : ""}`}
                        onClick={() => setSelectedActionIdx(idx)}
                      >
                        <span className="timeline-time">{action.at_ms}ms</span>
                        <span className="timeline-action">{formatAction(action.action)}</span>
                        <div className="timeline-actions">
                          <button
                            className="btn btn-sm"
                            onClick={(e) => { e.stopPropagation(); handleAdjustDelay(idx, -50); }}
                            title="-50ms"
                          >-</button>
                          <button
                            className="btn btn-sm"
                            onClick={(e) => { e.stopPropagation(); handleAdjustDelay(idx, 50); }}
                            title="+50ms"
                          >+</button>
                          <button
                            className="btn btn-sm"
                            onClick={(e) => { e.stopPropagation(); handleToggleAction(idx); }}
                            title={action.enabled ? "Disable" : "Enable"}
                          >{action.enabled ? "☑" : "☐"}</button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={(e) => { e.stopPropagation(); handleDeleteAction(idx); }}
                            title="Delete"
                          >🗑</button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}

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
        <span className={`status-state state-${engineState.toLowerCase()}`}>{engineState}</span>
        <span className="status-divider">|</span>
        <span className="status-value">{uiMessage || engineStatus}</span>
        {(isRunning || isPaused) && (
          <>
            <span className="status-divider">|</span>
            <span className="status-stats">{executedCount} actions, {iteration} iters</span>
          </>
        )}
      </footer>

    </div>
  );
}
