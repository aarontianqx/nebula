import { useState, useEffect } from 'react';
import { Play, Square, RefreshCw, PlayCircle, StopCircle, Timer } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ScriptInfo, SessionState, SessionStateEnum } from '../../types';
import { useSessionStore } from '../../stores/sessionStore';

interface Props {
  sessionId: string | null;
  sessionState: SessionState | null;
}

export default function ScriptControls({ sessionId, sessionState }: Props) {
  const [scripts, setScripts] = useState<ScriptInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [startAllDropdownOpen, setStartAllDropdownOpen] = useState(false);

  // Per-session script selection from store
  const { sessions, sessionScripts, setSessionScript } = useSessionStore();
  const selectedScript = sessionId ? (sessionScripts[sessionId] || '') : '';

  useEffect(() => {
    loadScripts();
  }, []);

  const loadScripts = async () => {
    try {
      const result = await invoke<ScriptInfo[]>('get_scripts');
      setScripts(result);
    } catch (e) {
      console.error('Failed to load scripts:', e);
    }
  };

  const handleScriptChange = (scriptName: string) => {
    if (sessionId) {
      setSessionScript(sessionId, scriptName);
    }
  };

  const isRunning = sessionState === SessionStateEnum.ScriptRunning;
  const canStart = sessionState === SessionStateEnum.Ready && selectedScript && sessionId;

  const handleStart = async () => {
    if (!sessionId || !selectedScript) return;
    setLoading(true);
    try {
      await invoke('start_script', { sessionId, scriptName: selectedScript });
    } catch (e) {
      console.error('Failed to start script:', e);
    }
    setLoading(false);
  };

  const handleStop = async () => {
    if (!sessionId) return;
    setLoading(true);
    try {
      await invoke('stop_script', { sessionId });
    } catch (e) {
      console.error('Failed to stop script:', e);
    }
    setLoading(false);
  };

  const handleRunAll = async () => {
    setLoading(true);
    try {
      // Pass sessionScripts map to backend - each session uses its own selected script
      await invoke('start_all_scripts', { sessionScripts });
    } catch (e) {
      console.error('Failed to start all scripts:', e);
    }
    setLoading(false);
  };

  const handleStopAll = async () => {
    setLoading(true);
    try {
      await invoke('stop_all_scripts');
    } catch (e) {
      console.error('Failed to stop all scripts:', e);
    }
    setLoading(false);
  };

  const handleRunAllStaggered = async () => {
    setStartAllDropdownOpen(false);
    setLoading(true);
    try {
      await invoke('start_all_scripts_staggered', { sessionScripts });
    } catch (e) {
      console.error('Failed to start all scripts (staggered):', e);
    }
    setLoading(false);
  };

  // Sync current session's script to all other sessions
  const handleSync = () => {
    if (!selectedScript) return;
    sessions.forEach((s) => {
      if (s.id !== sessionId) {
        setSessionScript(s.id, selectedScript);
      }
    });
  };

  return (
    <div className="flex items-center gap-2">
      {/* Configuration Area: Script Selection + Sync */}
      <div className="flex items-center gap-1">
        <select
          value={selectedScript}
          onChange={(e) => handleScriptChange(e.target.value)}
          className="border rounded px-3 py-2 bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] text-sm min-w-[140px] focus:outline-none focus:border-[var(--color-accent)]"
          disabled={isRunning || loading || !sessionId}
        >
          <option value="">Select Script</option>
          {scripts.map((s) => (
            <option key={s.name} value={s.name}>
              {s.name}
            </option>
          ))}
        </select>

        {/* Sync Button */}
        <button
          onClick={handleSync}
          disabled={loading || !selectedScript}
          className="flex items-center gap-1.5 px-2 py-2 border rounded bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors text-sm"
          title="Sync Script to All Sessions"
        >
          <RefreshCw className="w-4 h-4" />
          <span>Sync</span>
        </button>
      </div>

      {/* Divider */}
      <div className="h-6 w-px bg-[var(--color-border)]" />

      {/* Current Session Action */}
      {isRunning ? (
        <button
          onClick={handleStop}
          disabled={loading || !sessionId}
          className="flex items-center gap-1.5 px-3 py-2 bg-[var(--color-error)] text-white rounded hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity text-sm font-medium"
          title="Stop Script"
        >
          <Square className="w-4 h-4" />
          <span>Stop</span>
        </button>
      ) : (
        <button
          onClick={handleStart}
          disabled={!canStart || loading}
          className="flex items-center gap-1.5 px-3 py-2 bg-[var(--color-success)] text-white rounded hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity text-sm font-medium"
          title="Start Script"
        >
          <Play className="w-4 h-4" />
          <span>Start</span>
        </button>
      )}

      {/* Divider */}
      <div className="h-6 w-px bg-[var(--color-border)]" />

      {/* Global Actions */}
      {isRunning ? (
        <button
          onClick={handleStopAll}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-2 border rounded bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors text-sm"
          title="Stop All Scripts"
        >
          <StopCircle className="w-4 h-4" />
          <span>Stop All</span>
        </button>
      ) : (
        <div className="relative flex items-center">
          <button
            onClick={handleRunAll}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 border border-r-0 rounded-l bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors text-sm h-[34px]"
            title="Start All Scripts"
          >
            <PlayCircle className="w-4 h-4" />
            <span>Start All</span>
          </button>
          <button
            onClick={() => setStartAllDropdownOpen(!startAllDropdownOpen)}
            disabled={loading}
            className="px-1.5 border rounded-r bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors flex items-center h-[34px]"
            title="More options"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m6 9 6 6 6-6" /></svg>
          </button>
          {startAllDropdownOpen && (
            <>
              <div className="fixed inset-0 z-40" onClick={() => setStartAllDropdownOpen(false)} />
              <div className="absolute left-0 top-full mt-1 w-52 py-1 bg-[var(--color-bg-panel)] border border-[var(--color-border)] rounded-md shadow-lg z-50">
                <button
                  onClick={handleRunAllStaggered}
                  disabled={loading}
                  className="w-full px-3 py-2 text-sm text-left text-[var(--color-text-primary)] hover:bg-black/10 dark:hover:bg-white/10 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 transition-colors"
                >
                  <Timer size={14} />
                  Staggered Start All
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
