// Wires backend events to the Zustand stores. Call once on mount.

import { setCursorPos } from "../stores/cursorStore";
import { useEngineStore } from "../stores/engineStore";
import { useKeyClickStore } from "../stores/keyClickStore";
import { usePickStore } from "../stores/pickStore";
import { useRecorderStore } from "../stores/recorderStore";
import { useSimpleStore } from "../stores/simpleStore";
import { listen } from "./ipc";
import type { ColorResponse, EngineEvent, KeyClickEvent, PickedPosition, PickedWindow, Point, RecordingStatus } from "./types";

/** Route a position the picker reported: the field that asked for it, else Simple mode. */
export function routePickedPosition(x: number, y: number, color?: ColorResponse, window?: PickedWindow): void {
  if (!usePickStore.getState().deliverPosition(x, y, color, window)) {
    useSimpleStore.getState().setPickedPosition(x, y);
  }
}

export async function setupEventListeners(): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen<EngineEvent>("engine-event", (e) => useEngineStore.getState().handleEngineEvent(e.payload)),
    listen<void>("emergency-stop", () => useEngineStore.getState().onEmergencyStop()),
    listen<RecordingStatus>("recording-status", (e) => useRecorderStore.getState().setStatus(e.payload)),
    listen<Point>("mouse-position", (e) => setCursorPos(e.payload)),
    listen<PickedPosition>("position-picked", (e) => routePickedPosition(e.payload.x, e.payload.y, e.payload.color, e.payload)),
    listen<KeyClickEvent>("key-click-event", (e) => useKeyClickStore.getState().handleEvent(e.payload)),
  ]);

  return () => {
    for (const un of unlisteners) un();
  };
}
