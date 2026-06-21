// Shared domain types mirrored from the Rust backend (tap-core / src-tauri).
// These match the serde representations emitted over Tauri IPC.

export type EngineState = "Idle" | "Arming" | "Running" | "Paused" | "Stopped";
export type RecorderState = "Idle" | "Recording" | "Paused";
export type Mode = "simple" | "timeline";
export type TimelineView = "list" | "rail" | "code";
export type SimpleActionType = "click" | "key" | "key-to-click";

export type MouseButton = "Left" | "Right" | "Middle";

export interface Point {
  x: number;
  y: number;
}

// Discriminated union mirroring tap_core::Action (externally tagged enum).
export type ActionInfo =
  | { Click: { x: number; y: number; button: MouseButton } }
  | { DoubleClick: { x: number; y: number; button: MouseButton } }
  | { MouseDown: { x: number; y: number; button: MouseButton } }
  | { MouseUp: { x: number; y: number; button: MouseButton } }
  | { MouseMove: { x: number; y: number } }
  | { Drag: { from: Point; to: Point; duration_ms: number } }
  | { KeyTap: { key: string } }
  | { KeyDown: { key: string } }
  | { KeyUp: { key: string } }
  | { TextInput: { text: string } }
  | { Wait: { ms: number } }
  | { Scroll: { delta_x: number; delta_y: number } };

export type ActionKind = keyof UnionToIntersection<ActionInfo>;

type UnionToIntersection<U> = (U extends unknown ? (k: U) => void : never) extends (k: infer I) => void
  ? I
  : never;

export interface TimedAction {
  at_ms: number;
  action: ActionInfo;
  enabled: boolean;
  note: string | null;
}

export interface Timeline {
  actions: TimedAction[];
}

export type Repeat = { Times: number } | "Forever";

export interface RunConfig {
  start_delay_ms: number;
  speed: number;
  repeat: Repeat;
}

export interface TargetWindow {
  title: string | null;
  process: string | null;
  pause_when_unfocused: boolean;
}

export interface Profile {
  name: string;
  timeline: Timeline;
  run: RunConfig;
  target_window: TargetWindow | null;
}

export interface WindowInfoResponse {
  handle: number;
  title: string;
  process_name: string;
  pid: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ColorResponse {
  r: number;
  g: number;
  b: number;
  hex: string;
}

export interface ValidationErrorResponse {
  path: string;
  message: string;
  line: number | null;
}

export interface VariableDefinitionResponse {
  name: string;
  var_type: string;
  default: unknown;
  description: string | null;
}

export interface TemplateInfo {
  id: string;
  name: string;
  description: string | null;
}

export interface DocumentMeta {
  description: string | null;
  author: string | null;
  tags: string[];
}

export interface RecordingStatus {
  state: RecorderState;
  event_count: number;
  duration_ms: number;
}

export interface LogEntry {
  time: string;
  message: string;
}

export type EngineEvent =
  | { StateChanged: { old: EngineState; new: EngineState } }
  | { CountdownTick: { remaining_secs: number } }
  | { ActionStarting: { index: number; action: ActionInfo } }
  | { ActionCompleted: { index: number } }
  | { IterationCompleted: { iteration: number } }
  | "Completed"
  | { Error: { message: string } }
  | { WaitingForCondition: { condition: string } }
  | { ConditionSatisfied: { condition: string } }
  | { ConditionTimeout: { condition: string } }
  | { CounterChanged: { key: string; value: number } }
  | { TargetWindowUnfocused: { title: string | null; process: string | null } }
  | "TargetWindowFocused";

export type KeyClickEvent =
  | "Started"
  | { Click: { count: number; x: number; y: number } }
  | { Stopped: { total_clicks: number } };
