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
│  - Storage                │  YAML document persistence
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
├── src-tauri/src/                # Tauri backend (thin adapter)
│   ├── main.rs                   #   builder + command registration only
│   ├── app.rs                    #   setup (wires engine, recorder, trackers) + emergency stop
│   ├── ports.rs                  #   engine port implementations (executor, platform provider)
│   ├── events.rs                 #   event pump: engine/recorder/tracker events -> frontend
│   ├── state.rs                  #   AppState + event payloads
│   ├── picker.rs, key_click.rs, templates.rs
│   └── commands/                 #   #[tauri::command] handlers grouped by concern
│                                 #   (engine, document, yaml, recording, key_click, system)
├── crates/
│   ├── tap-core/                 # Pure domain: model, DSL, conditions, variables, expression, schema
│   ├── tap-application/          # Application layer: Coordinator, SessionStore, Player, Recorder, Resolve, storage, ports
│   └── tap-platform/             # Platform abstraction (input injection, hooks, window/pixel/DPI)
├── templates/                    # YAML macro templates
├── specs/
│   ├── features/                 # Feature specifications
│   └── proposals/                # backlog.md — forward-looking backlog / future work
└── README.md
```

### Key conventions

- **Engine decoupling**: The automation engine lives in `tap-application` and depends only on the pure domain (`tap-core`) plus trait *ports*; it must stay independent of the UI framework and the OS. `src-tauri` is a thin adapter that wires up concrete port implementations (the `tap-platform` injector / window queries) and forwards IPC + events. Swapping UI or platform should not affect engine logic.
- **Single source of truth**: The canonical macro is the `MacroDocument` held by `SessionStore`; the resolved `Profile` exists only inside the engine's Resolve stage. The UI edits the document itself over IPC (`get_document` / `update_document`), so there is no lossy projection to write back and a new document field cannot be silently reset by a bridge.
- **Adapter layout**: `src-tauri/src/main.rs` stays a registration list. A new IPC command goes in the matching `commands/<concern>.rs` (or a new module if it is a new concern), is registered in `main.rs` by its module path, and keeps no logic of its own beyond translating to/from the application layer. Shared mutable resources live in `AppState`, not in module-level statics. Keep modules small (rule of thumb: under ~300 lines).
- **Serial execution**: Timeline actions execute sequentially within a single macro. No concurrent injection.
- **Cancellability**: All long-running operations (replay, record) must support immediate cancellation via stop signal.
- **Global coordinate system**: Mouse coordinates use the OS *injection* space — full-screen **physical pixels on Windows**, **points on macOS** — never WebView-relative coordinates. The picker converts browser CSS pixels via `browser_to_injection_scale()` so recording, playback and picking stay consistent. `get_primary_scale_factor()` reports the display backing scale and is informational only.
- **Safety first**: Emergency stop (global hotkey `Ctrl+Shift+Backspace`) has the highest priority and must work regardless of window focus.

### Frontend architecture (`src/`)

The React UI is **store-driven**; components stay thin and read/write Zustand stores:

| Store | Responsibility |
|-------|----------------|
| `documentStore` | Editable mirror of the canonical macro (name/timeline/run/target), profile list, debounced backend sync |
| `engineStore` | Engine run state, countdown, run stats, activity log, lifecycle controls |
| `recorderStore` | Recording state + controls |
| `toolStore` | Simple-mode config, Key→Click, position picker, color sampling |
| `pickStore` | One-shot requests from editor fields: pick a screen position, capture a key |
| `uiStore` | View mode, timeline view (List/Rail/Code), selection, modal, code buffer |

- **IPC boundary**: every backend call goes through `lib/ipc.ts` (a typed `api`, guarded so the UI still renders without the Tauri runtime). Backend events are wired to stores once in `lib/events.ts`.
- **Sync contract (edits must persist)**: `documentStore` holds `doc: MacroDocument`, a mirror of the backend's canonical document. Visual edits update the mirror and **debounce-push** the whole document via `update_document`; pushes are serialized, and the pending push is **force-flushed before Start, Save, Export and opening the Code view**, so playback/persistence always use the latest edits. Record/import/load/template replace the mirror with the backend's document (`applyDocument` drops any pending push). This is what makes "edit → save/replay reflects it" structurally guaranteed.
- **Edits are never rejected**: `update_document` stores whatever the UI sends (the editor passes through states that are not runnable yet, such as an empty timeline) and returns validation findings as non-blocking warnings shown inline. Import paths (YAML/file/template) still refuse invalid documents.
- **JSON contract**: the TypeScript document types in `src/lib/types.ts` are hand-written. `crates/tap-core/tests/document_json_contract.rs` serializes a document using every action and condition variant to `src/lib/__fixtures__/kitchen_sink.document.json` (regenerate with `UPDATE_FIXTURES=1 cargo test -p tap-core --test document_json_contract`), and `documentContract.test.ts` checks the types against it, so drift on either side fails a test. serde omits defaults (`enabled: true`, empty tags/variables), so read optional fields through `lib/document.ts`.
- **Inspector coverage**: every action kind is edited parameter by parameter (`ActionParams.tsx`, recursive for the branches of a `conditional`); leaf conditions are edited field by field (`ConditionEditor.tsx`), and only `and` / `or` / `not` are left to the **Code (YAML)** view. Adding a variant to `DslAction` / `DslCondition` fails `tsc` until `lib/actions.ts` / `lib/conditions.ts` and the editors handle it. Value fields (`ValueInput` in `fields.tsx`) hold a literal or a `{{ }}` template: `lib/expression.ts` converts text to `DslValue` and lints templates (advisory only), `lib/document.ts` lists the variables and counters a template may use. `call_macro` is a timeline step only (the engine does not expand it inside an `if`), which both the nested-action editor and `validate_profile` enforce.
- **Pick / Capture from the Inspector**: the picker window reports through one global event (`position-picked`) that Simple mode also uses, so `pickStore` remembers which field asked and `lib/events.ts` (`routePickedPosition`) gives the position to that field first, to Simple mode only when nobody asked; `toolStore.openPicker` drops a stale request (a picker closed with Esc reports nothing). A request or capture is cancelled when its field unmounts, and a captured key is applied to the step as it is *then*. Pick is disabled while a coordinate is an expression (it would be overwritten). `cmd_capture_key` waits up to 10 s, so it is `#[tauri::command(async)]`: a plain command runs on the main thread and would freeze the window for that long.
- **Frontend tests**: `yarn test` (vitest, needs Node 22+). Store tests run against a mocked IPC layer (`src/test/`); reset module-level store state with `vi.resetModules()` + dynamic import.
- **Timeline editor**: List view (add / insert / delete / move / duplicate / retime / toggle / note), Rail view (drag-to-retime), and Code (YAML) view, with an Inspector for per-action parameter editing (see Inspector coverage above).

### Platform abstraction (`tap-platform`)

| Submodule        | Responsibility               | Windows        | macOS                   |
|------------------|------------------------------|----------------|-------------------------|
| `injector.rs`    | Mouse/keyboard injection     | enigo          | enigo                   |
| `events/`        | Global event listener (singleton) | N/A       | CGEventTap + subscriptions |
| `input_hook/`    | Global input hook (recording)| rdev           | Subscribes to events singleton |
| `mouse_tracker/` | Global mouse position        | rdev           | Subscribes to events singleton |
| `window/`        | Window API                   | Win32 API      | Quartz `CGWindowList` + `NSWorkspace` (frontmost app) |
| `pixel/`         | Pixel color reading          | GDI            | `CGWindowListCreateImage` |
| `dpi/`           | High DPI handling            | SetProcessDpiAwareness | backing scale via `CGDisplayMode` |

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
