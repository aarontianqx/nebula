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

/** What a screen pick reports: where, and the color there when the pick asked for it. */
export type PickMode = "position" | "position_color";

export interface PickedPosition extends Point {
  /** Present only for `position_color` picks, and absent if the color could not be read. */
  color?: ColorResponse;
  /** The target window's top-left corner, when the document measures positions from that window. */
  window_origin?: Point;
  /** The document measures from its target window, but the window was not found. */
  window_missing?: boolean;
}

/** What the picker knows about the document's target window (see `PickedPosition`). */
export type PickedWindow = Pick<PickedPosition, "window_origin" | "window_missing">;

// === Resolved actions ===
//
// `tap_core::Action` with every value concrete. These are what the *engine* reports
// (`ActionStarting`) and what the recorder produces; they are not what the editor edits.
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

export interface TimedAction {
  at_ms: number;
  action: ActionInfo;
  enabled: boolean;
  note: string | null;
}

export interface Timeline {
  actions: TimedAction[];
}

// === The document model ===
//
// Mirrors `tap_core::MacroDocument` as serialized by serde_json: this is the exact payload of
// `get_document` / `update_document`. The Rust test `document_json_contract` writes a fixture that
// uses every variant; `documentContract.test.ts` checks these types against it.
//
// Wire conventions worth knowing:
// - unit variants (`"exit"`, `"always"`, `"never"`) serialize as bare strings, every other
//   variant is `{ <snake_case_name>: { ...fields } }`;
// - `None` / empty collections / `enabled: true` are omitted, so the optional fields below may
//   simply be absent. Never send `null` for the non-`Option` ones (`tags`, `variables`, `enabled`).

/** A literal or a `"{{ expression }}"` string. Integers and floats are both `number`. */
export type DslValue = number | string | boolean;

export type DslMouseButton = "left" | "right" | "middle";

export type DslCondition =
  | { window_focused: { title?: string | null; process?: string | null } }
  | { window_exists: { title?: string | null; process?: string | null } }
  | { pixel_color: { x: number; y: number; color: string; tolerance: number } }
  | { counter: { key: string; op: string; value: number } }
  | "always"
  | "never"
  | { and: DslCondition[] }
  | { or: DslCondition[] }
  | { not: { condition: DslCondition } };

export type DslAction =
  | { click: { x: DslValue; y: DslValue; button: DslMouseButton } }
  | { double_click: { x: DslValue; y: DslValue; button: DslMouseButton } }
  | { mouse_down: { x: DslValue; y: DslValue; button: DslMouseButton } }
  | { mouse_up: { x: DslValue; y: DslValue; button: DslMouseButton } }
  | { mouse_move: { x: DslValue; y: DslValue } }
  | { drag: { from_x: DslValue; from_y: DslValue; to_x: DslValue; to_y: DslValue; duration_ms: number } }
  | { scroll: { delta_x: DslValue; delta_y: DslValue } }
  | { key_tap: { key: string } }
  | { key_down: { key: string } }
  | { key_up: { key: string } }
  | { text_input: { text: DslValue } }
  | { wait: { ms: number } }
  | { wait_until: { condition: DslCondition; timeout_ms?: number | null; poll_interval_ms: number } }
  | { conditional: { condition: DslCondition; then_action: DslAction; else_action?: DslAction | null } }
  | { set_counter: { key: string; value: DslValue } }
  | { incr_counter: { key: string } }
  | { decr_counter: { key: string } }
  | { reset_counter: { key: string } }
  | "exit"
  | { call_macro: { name: string; args?: Record<string, DslValue> } };

type UnionToIntersection<U> = (U extends unknown ? (k: U) => void : never) extends (k: infer I) => void
  ? I
  : never;

/** The discriminant of a [`DslAction`]: its variant name. */
export type DslActionKind = Extract<DslAction, string> | keyof UnionToIntersection<Exclude<DslAction, string>>;

/** The discriminant of a [`DslCondition`]: its variant name. */
export type DslConditionKind =
  | Extract<DslCondition, string>
  | keyof UnionToIntersection<Exclude<DslCondition, string>>;

export interface DslTimedAction {
  /** Earliest start in ms on the active-time clock (see dsl-reference.md "Timing semantics"). */
  at_ms: number;
  action: DslAction;
  /** Absent means enabled. */
  enabled?: boolean;
  note?: string | null;
}

export interface DslRunConfig {
  /** Number of runs; `0` means forever. */
  repeat: number;
  start_delay_ms: number;
  speed: number;
  /** Random variation of positions and step timing. Absent means none. */
  humanize?: Humanize;
}

/** `run.humanize`: how far (at most) playback strays from the recorded positions and times. */
export interface Humanize {
  /** Pixels, per axis. */
  position_px: number;
  /** Milliseconds, earlier or later. */
  delay_ms: number;
}

/** What a document's coordinates are measured from. */
export type CoordinateSpace = "screen" | "window";

export interface DslTargetWindow {
  title?: string | null;
  process?: string | null;
  pause_when_unfocused: boolean;
  /** Absent means `"screen"`. */
  coordinates?: CoordinateSpace;
}

export type VariableType = "string" | "number" | "boolean";

export interface VariableDefinition {
  type: VariableType;
  default?: unknown;
  description?: string | null;
}

export interface MacroDocument {
  name: string;
  description?: string | null;
  version: string;
  author?: string | null;
  tags?: string[];
  variables?: Record<string, VariableDefinition>;
  target_window?: DslTargetWindow | null;
  timeline: DslTimedAction[];
  run: DslRunConfig;
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

export interface TemplateInfo {
  id: string;
  name: string;
  description: string | null;
}

export interface PermissionStatus {
  /** Global input hook + injection allowed (macOS Accessibility; true where N/A). */
  accessibility: boolean;
  /** Screen capture for pixel/window reads allowed (macOS Screen Recording; true where N/A). */
  screen_recording: boolean;
  /** Target OS, so the UI can show platform-specific guidance. */
  os: string;
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

export type KeyClickLocationMode = "cursor" | "fixed";

export interface KeyClickRequest {
  minIntervalMs: number;
  holdDelayMs: number;
  button: "left" | "right" | "middle";
  locationMode: KeyClickLocationMode;
  fixedX: number;
  fixedY: number;
  onlyTargetFocused: boolean;
}

export type KeyClickEvent =
  | "Started"
  | { Click: { count: number; x: number; y: number } }
  | { Stopped: { total_clicks: number; reason: string } };
