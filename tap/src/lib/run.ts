// Shared "start a timeline run" sequence, used by both the Play button and the
// run-before variable form. Kept in a standalone module so it can pull from the
// document and engine stores without creating an import cycle between them.

import { useDocumentStore } from "../stores/documentStore";
import { useEngineStore } from "../stores/engineStore";
import { api } from "./ipc";

/**
 * Force the latest edits to the canonical document, then start playback.
 *
 * Run-time variable overrides (if any) must already have been pushed via
 * `cmd_set_runtime_variables` before calling this.
 */
export async function startTimelineRun(): Promise<void> {
  const engine = useEngineStore.getState();
  try {
    engine.resetRunStats();
    await useDocumentStore.getState().flush();
    await api.startExecution();
    engine.addLog("Playing timeline");
  } catch (err) {
    engine.setStatus(`Failed: ${String(err)}`);
    engine.addLog(`Error: ${String(err)}`);
  }
}
