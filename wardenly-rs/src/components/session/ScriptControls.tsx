import { useState, useEffect } from 'react';
import { Play, Square, RefreshCw, PlayCircle, StopCircle } from 'lucide-react';
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
      {/* Script Selection */}
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

      {/* Start/Stop Button */}
      {isRunning ? (
        <button
          onClick={handleStop}
          disabled={loading || !sessionId}
          className="p-2 bg-[var(--color-error)] text-white rounded hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
          title="Stop Script"
        >
          <Square className="w-4 h-4" />
        </button>
      ) : (
        <button
          onClick={handleStart}
          disabled={!canStart || loading}
          className="p-2 bg-[var(--color-success)] text-white rounded hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
          title="Start Script"
        >
          <Play className="w-4 h-4" />
        </button>
      )}

      {/* Start All / Stop All Scripts Toggle Button */}
      {isRunning ? (
        <button
          onClick={handleStopAll}
          disabled={loading}
          className="p-2 border rounded bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors"
          title="Stop All Scripts"
        >
          <StopCircle className="w-4 h-4" />
        </button>
      ) : (
        <button
          onClick={handleRunAll}
          disabled={loading}
          className="p-2 border rounded bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors"
          title="Start All Scripts"
        >
          <PlayCircle className="w-4 h-4" />
        </button>
      )}

      {/* Sync Button - copy current script to all sessions */}
      <button
        onClick={handleSync}
        disabled={loading || !selectedScript}
        className="p-2 border rounded bg-[var(--color-bg-secondary)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:opacity-50 transition-colors"
        title="Sync Script to All Sessions"
      >
        <RefreshCw className="w-4 h-4" />
      </button>
    </div>
  );
}
