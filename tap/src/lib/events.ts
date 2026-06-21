// Wires backend events to the Zustand stores. Call once on mount.

import { useEngineStore } from "../stores/engineStore";
import { useRecorderStore } from "../stores/recorderStore";
import { useToolStore } from "../stores/toolStore";
import { listen } from "./ipc";
import type { EngineEvent, KeyClickEvent, Point, RecordingStatus } from "./types";

export async function setupEventListeners(): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen<EngineEvent>("engine-event", (e) => useEngineStore.getState().handleEngineEvent(e.payload)),
    listen<void>("emergency-stop", () => useEngineStore.getState().onEmergencyStop()),
    listen<RecordingStatus>("recording-status", (e) => useRecorderStore.getState().setStatus(e.payload)),
    listen<Point>("mouse-position", (e) => useEngineStore.getState().setMousePos(e.payload)),
    listen<Point>("position-picked", (e) => useToolStore.getState().setPickedPosition(e.payload.x, e.payload.y)),
    listen<KeyClickEvent>("key-click-event", (e) => useToolStore.getState().handleKeyClickEvent(e.payload)),
  ]);

  return () => {
    for (const un of unlisteners) un();
  };
}
