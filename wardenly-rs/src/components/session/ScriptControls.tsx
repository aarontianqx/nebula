import { useState, useEffect } from 'react';
import { Play, Square, RefreshCw, PlayCircle, StopCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ScriptInfo, SessionState, SessionStateEnum } from '../../types';

interface Props {
  sessionId: string | null;
  sessionState: SessionState | null;
}

export default function ScriptControls({ sessionId, sessionState }: Props) {
  const [scripts, setScripts] = useState<ScriptInfo[]>([]);
  const [selectedScript, setSelectedScript] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadScripts();
  }, []);

  const loadScripts = async () => {
    try {
      const result = await invoke<ScriptInfo[]>('get_scripts');
      setScripts(result);
      if (result.length > 0 && !selectedScript) {
        setSelectedScript(result[0].name);
      }
    } catch (e) {
      console.error('Failed to load scripts:', e);
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

  const handleSync = () => {
    // Sync script selection to all sessions (future feature)
    console.log('Sync script selection:', selectedScript);
  };

  const handleRunAll = async () => {
    if (!selectedScript) return;
    setLoading(true);
    try {
      await invoke('start_all_scripts', { scriptName: selectedScript });
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

  return (
    <div className="flex items-center gap-2">
      {/* Script Selection */}
      <select
        value={selectedScript}
        onChange={(e) => setSelectedScript(e.target.value)}
        className="border rounded px-3 py-2 bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 text-sm min-w-[140px]"
        disabled={isRunning || loading}
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
          className="p-2 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50 disabled:cursor-not-allowed"
          title="Stop Script"
        >
          <Square className="w-4 h-4" />
        </button>
      ) : (
        <button
          onClick={handleStart}
          disabled={!canStart || loading}
          className="p-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50 disabled:cursor-not-allowed"
          title="Start Script"
        >
          <Play className="w-4 h-4" />
        </button>
      )}

      {/* Sync Button */}
      <button
        onClick={handleSync}
        disabled={loading}
        className="p-2 border rounded bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700"
        title="Sync Script Selection"
      >
        <RefreshCw className="w-4 h-4" />
      </button>

      {/* Run All Button */}
      <button
        onClick={handleRunAll}
        disabled={!selectedScript || loading}
        className="p-2 border rounded bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-50"
        title="Run All Sessions"
      >
        <PlayCircle className="w-4 h-4" />
      </button>

      {/* Stop All Button */}
      <button
        onClick={handleStopAll}
        disabled={loading}
        className="p-2 border rounded bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700"
        title="Stop All Scripts"
      >
        <StopCircle className="w-4 h-4" />
      </button>
    </div>
  );
}

