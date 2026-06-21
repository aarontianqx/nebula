# tap -- Agent Guidelines

## Overview

tap (Timed Action Performer) is a cross-platform desktop automation tool built with Tauri v2 + React + Rust. It provides progressive automation capabilities: repeat → record/replay → conditions → scripting → plugins.

## Architecture

### Layered design

```
┌───────────────────────────┐
│    Presentation (GUI)     │  Tauri (React + Vite)
│  - Timeline editor        │
│  - Profile manager        │
└──────────────┬────────────┘
               │ Commands (Start/Stop/Record/Replay)
               ▼
┌───────────────────────────┐
│ Application (tap-application) │
│  - Coordinator            │  Entry point: owns player + session
│  - SessionStore           │  Canonical MacroDocument (single source of truth)
│  - Player (Replay)        │  Document playback + Resolve stage (interruptible)
│  - Recorder (Record)      │  Global event capture (pausable)
│  - Storage                │  YAML persistence (+ legacy JSON read)
└──────────────┬────────────┘
               │ Ports (traits: ActionExecutor / PlatformConditionProvider)
               ▼
┌───────────────────────────┐
│ Infrastructure (OS I/O)   │
│  - InputInjector           │  Mouse/keyboard injection
│  - InputHook               │  Global event listener
│  - Window / Pixel / DPI    │  Platform queries
└──────────────┬────────────┘
               ▼
┌───────────────────────────┐
│        Domain             │
│  - Action / Timeline      │  Data structures and validation
│  - Conditions             │  Condition evaluation
│  - Variables              │  Variable/counter store
│  - DSL                    │  YAML serialization/deserialization
│  - Expression             │  Rhai sandboxed expression engine
└───────────────────────────┘
```

### Directory structure

```
tap/
├── src/                          # React frontend (Vite)
├── src-tauri/src/                # Tauri backend (IPC commands)
├── crates/
│   ├── tap-core/                 # Pure domain: model, DSL, conditions, variables, expression, schema
│   ├── tap-application/          # Application layer: Coordinator, SessionStore, Player, Recorder, Resolve, storage, ports
│   └── tap-platform/             # Platform abstraction (input injection, hooks, window/pixel/DPI)
├── templates/                    # YAML macro templates
├── specs/
│   ├── features/                 # Feature specifications
│   └── proposals/                # Design proposals and RFCs
└── README.md
```

### Key conventions

- **Engine decoupling**: The automation engine lives in `tap-application` and depends only on the pure domain (`tap-core`) plus trait *ports*; it must stay independent of the UI framework and the OS. `src-tauri` is a thin adapter that wires up concrete port implementations (the `tap-platform` injector / window queries) and forwards IPC + events. Swapping UI or platform should not affect engine logic.
- **Single source of truth**: The canonical macro is the `MacroDocument` held by `SessionStore`; the runtime `Profile` is a resolved projection for display/IPC. Edits and execution always go through the document, never a lossy side copy.
- **Serial execution**: Timeline actions execute sequentially within a single macro. No concurrent injection.
- **Cancellability**: All long-running operations (replay, record) must support immediate cancellation via stop signal.
- **Global coordinate system**: All mouse coordinates use full-screen physical pixels, never WebView-relative coordinates.
- **Safety first**: Emergency stop (global hotkey `Ctrl+Shift+Backspace`) has the highest priority and must work regardless of window focus.

### Platform abstraction (`tap-platform`)

| Submodule        | Responsibility               | Windows        | macOS                   |
|------------------|------------------------------|----------------|-------------------------|
| `injector.rs`    | Mouse/keyboard injection     | enigo          | enigo                   |
| `events/`        | Global event listener (singleton) | N/A       | CGEventTap + subscriptions |
| `input_hook/`    | Global input hook (recording)| rdev           | Subscribes to events singleton |
| `mouse_tracker/` | Global mouse position        | rdev           | Subscribes to events singleton |
| `window/`        | Window API                   | Win32 API      | (pending)               |
| `pixel/`         | Pixel color reading          | GDI            | (pending)               |
| `dpi/`           | High DPI handling            | SetProcessDpiAwareness | NSScreen scale |

macOS uses a CGEventTap singleton shared across `mouse_tracker` and `input_hook` to avoid multiple event tap conflicts.

### Tool Modes vs Timeline

Some features are **event-driven tool modes** (driven by live global input, not a pre-defined timeline). Tool modes are out of scope for the DSL and cannot be expressed as YAML actions.

Example: **Key→Click** -- holding A-Z triggers repeated mouse clicks; Space stops immediately. See `specs/features/key-to-click-mode.md`.

### Expression engine

Rhai is used for sandboxed expression evaluation (`{{ counter + 1 }}`). File and network access are disabled, execution depth and operand count are limited.

### Spec references

| Topic                     | Spec                                        |
|---------------------------|---------------------------------------------|
| Core features & concepts  | `specs/features/functional-guide.md`         |
| UI/UX design              | `specs/features/ui-design.md`                |
| YAML DSL syntax           | `specs/features/dsl-reference.md`            |
| Key→Click tool mode       | `specs/features/key-to-click-mode.md`        |
