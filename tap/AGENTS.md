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
│  - MacroLibrary           │  Saved macros as YAML on disk
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
│  - MacroDocument / Action │  Data structures and validation
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
│   ├── app.rs                    #   setup (builds and manages each state) + emergency stop
│   ├── ports.rs                  #   engine port implementations (executor, platform provider)
│   ├── forward.rs                #   threads relaying engine / mouse events to the frontend
│   ├── state.rs                  #   managed state types, `ensure_idle`
│   ├── dto.rs                    #   wire types only the adapter owns
│   ├── recording.rs, key_click.rs #  input hook <-> RecordingSession / KeyClickMachine
│   ├── picker.rs, templates.rs
│   └── commands/                 #   #[tauri::command] handlers grouped by concern
│                                 #   (engine, document, yaml, recording, key_click, system)
├── crates/
│   ├── tap-core/                 # Pure domain: model, DSL, conditions, variables, expression, schema
│   ├── tap-application/          # Application layer: Coordinator, SessionStore, playback (Player + ports + protocol), Recorder, Resolve, MacroLibrary (YAML persistence)
│   └── tap-platform/             # Platform abstraction (input injection, hooks, window/pixel/DPI)
├── templates/                    # YAML macro templates
├── specs/
│   ├── features/                 # Feature specifications
│   └── proposals/                # backlog.md — forward-looking backlog / future work
└── README.md
```

### Key conventions

- **Engine decoupling**: The automation engine (`tap-application/src/playback/`: `protocol` commands/events, `ports` traits, `player` thread and waits, `run` passes and scheduling, `dispatch` single actions) lives in `tap-application` and depends only on the pure domain (`tap-core`) plus trait *ports*; it must stay independent of the UI framework, the OS and storage (the macros `call_macro` names are found through a loader injected via `PlayerConfig`). `MacroLibrary` is built by the adapter with an explicit root (`default_root()` in production), so no module reaches for a global path; the on-disk names (`profiles/`, `.yaml`) and the `cmd_*_profile` IPC commands keep the pre-rename vocabulary on purpose. A `Start` command carries the document and run-time variables, so the player keeps no copy of them and the coordinator reads the player's state instead of mirroring it. `src-tauri` is a thin adapter that wires up concrete port implementations (the `tap-platform` injector / window queries) and forwards IPC + events. Swapping UI or platform should not affect engine logic.
- **Single source of truth**: The canonical macro is the `MacroDocument` held by `SessionStore`; the engine turns each templated step into a concrete `Action` only at run time (the Resolve stage). The UI edits the document itself over IPC (`get_document` / `update_document`), so there is no lossy projection to write back and a new document field cannot be silently reset by a bridge. Runs that are not the editor's macro (the Simple-mode repeat tool) play a throwaway document through `Coordinator::start_standalone`, so they can never overwrite or diverge from the session document.
- **Adapter layout**: `src-tauri/src/main.rs` stays a registration list. A new IPC command goes in the matching `commands/<concern>.rs` (or a new module if it is a new concern), is registered in `main.rs` by its module path, and keeps no logic of its own beyond translating to/from the application layer. Shared state is split into one Tauri-managed type per concern (`CoordinatorState`, `RecordingState`, `KeyClickState`, `DryRun`, `MacroLibrary`), so a command locks only what it uses and nothing lives in module-level statics. Events are pushed by per-source threads blocking on their channel; there is no polling loop. Decisions about *what* to record or *when* to click live in `tap-application` (`RecordingSession`, `KeyClickMachine`) where they are unit-tested; the adapter only does the I/O. Keep modules small (rule of thumb: under ~300 lines).
- **Serial execution**: Timeline actions execute sequentially within a single macro. No concurrent injection.
- **Cancellability**: All long-running operations (replay, record) must support immediate cancellation via stop signal.
- **Global coordinate system**: Mouse coordinates use the OS *injection* space — full-screen **physical pixels on Windows**, **points on macOS** — never WebView-relative coordinates. The picker converts browser CSS pixels via `browser_to_injection_scale()` so recording, playback and picking stay consistent. A document with `target_window.coordinates: window` measures positions from its target window's top-left corner instead: the engine adds the window's *current* origin (`PlatformConditionProvider::window_origin`, resolved by `tap_platform::pick_target_window`: focused match first, else largest) when it dispatches each positioned action and reads each `pixel_color`, the recorder subtracts it per event (looked up fresh for button events, cached ≤100 ms for moves), and the picker event carries `window_origin` for the editor to subtract. A missing window is waited for like an unfocused one. Shifting lives in `tap-application/src/coords.rs`; `has_position` is an exhaustive match so a new positioned action cannot be missed. `run.humanize` (`tap-application/src/playback/humanize.rs`) varies positions when an action is dispatched (before the window shift) and step start times when the timeline is scheduled; press/release pairs share an offset, conditions are never varied, and the seed is injectable (`PlayerConfig::with_seed`) for reproducible tests.
- **Safety first**: Emergency stop (global hotkey `Ctrl+Shift+Backspace`) has the highest priority and must work regardless of window focus.

### Frontend architecture (`src/`)

The React UI is **store-driven**; components stay thin and read/write Zustand stores:

| Store | Responsibility |
|-------|----------------|
| `documentStore` | Editable mirror of the canonical macro (name/timeline/run/target) and its debounced backend sync (`lib/debouncedPush.ts`) |
| `libraryStore` | Saved macros, recents and templates; loading, deleting, importing and exporting them (replaces the document through `documentStore`) |
| `engineStore` | Engine run state, countdown, run stats, lifecycle controls |
| `recorderStore` | Recording state + controls |
| `simpleStore` | Simple tab: one repeated click / key; also holds the position the picker fills |
| `keyClickStore` | Key→Click tool (fixed-position mode reuses `simpleStore`'s position) |
| `colorStore` | Color read under the cursor |
| `pickStore` | One-shot requests from editor fields: pick a screen position, capture a key |
| `cursorStore` | Pointer position reported by the backend |
| `logStore` | Activity log. A leaf: any store may `log()`, it depends on none |
| `uiStore` | View mode, timeline view (List/Rail/Code), selection, modal, code buffer |

- **IPC boundary**: every backend call goes through `lib/ipc.ts` (a typed `api`, guarded so the UI still renders without the Tauri runtime). Backend events are wired to stores once in `lib/events.ts`.
- **Sync contract (edits must persist)**: `documentStore` holds `doc: MacroDocument`, a mirror of the backend's canonical document. Visual edits update the mirror and **debounce-push** the whole document via `update_document`; pushes are serialized, and the pending push is **force-flushed before Start, Save, Export and opening the Code view**, so playback/persistence always use the latest edits. Record/import/load/template replace the mirror with the backend's document (`applyDocument` drops any pending push). This is what makes "edit → save/replay reflects it" structurally guaranteed.
- **Edits are never rejected**: `update_document` stores whatever the UI sends (the editor passes through states that are not runnable yet, such as an empty timeline) and returns validation findings as non-blocking warnings shown inline. Import paths (YAML/file/template) still refuse invalid documents.
- **JSON contract**: the TypeScript document types in `src/lib/types.ts` are hand-written. `crates/tap-core/tests/document_json_contract.rs` serializes a document using every action and condition variant to `src/lib/__fixtures__/kitchen_sink.document.json` (regenerate with `UPDATE_FIXTURES=1 cargo test -p tap-core --test document_json_contract`), and `documentContract.test.ts` checks the types against it, so drift on either side fails a test. serde omits defaults (`enabled: true`, empty tags/variables), so read optional fields through `lib/document.ts`.
- **Inspector coverage**: every action kind is edited parameter by parameter (`ActionParams.tsx`, recursive for the branches of a `conditional`); leaf conditions are edited field by field (`ConditionEditor.tsx`), and only `and` / `or` / `not` are left to the **Code (YAML)** view. Adding a variant to `DslAction` / `DslCondition` fails `tsc` until `lib/actions.ts` / `lib/conditions.ts` and the editors handle it. Value fields (`ValueInput` in `fields.tsx`) hold a literal or a `{{ }}` template: `lib/expression.ts` converts text to `DslValue` and lints templates (advisory only), `lib/document.ts` lists the variables and counters a template may use. The engine runs a step on its DSL action (`Player::run_step`): `call_macro` and `conditional` are decided there, so a `call_macro` works in either branch of an `if`, at any depth, and only the branch taken is resolved.
- **Pick / Capture from the Inspector**: the picker window reports through one global event (`position-picked`) that Simple mode also uses, so `pickStore` remembers which field asked and `lib/events.ts` (`routePickedPosition`) gives the position to that field first, to Simple mode only when nobody asked; `simpleStore.openPicker` drops a stale request (a picker closed with Esc reports nothing). A request or capture is cancelled when its field unmounts, and a captured key is applied to the step as it is *then*. Pick is disabled while a coordinate is an expression (it would be overwritten). A `pixel_color` pick asks for `position_color` (`PickMode`, held in the backend's `PickerState` and reset by every `open_picker_window` call): `picker_position_selected` closes the overlay, waits for it to leave the screen plus one repaint, samples in injection space, and returns `{x, y, color}`; the color is absent when it cannot be read, and the request then logs that only the position was set. `cmd_capture_key` waits up to 10 s, so it is `#[tauri::command(async)]`: a plain command runs on the main thread and would freeze the window for that long.
- **Frontend tests**: `yarn test` (vitest, needs Node 22.12+). Store tests run against a mocked IPC layer (`src/test/`); reset module-level store state with `vi.resetModules()` + dynamic import.
- **Timeline editor**: List view (add / insert / delete / move / duplicate / retime / toggle / note), Rail view (drag-to-retime), and Code (YAML) view, with an Inspector for per-action parameter editing (see Inspector coverage above).

### Platform abstraction (`tap-platform`)

| Submodule        | Responsibility               | Windows        | macOS                   |
|------------------|------------------------------|----------------|-------------------------|
| `injector.rs`    | Mouse/keyboard injection     | enigo          | enigo                   |
| `events/`        | Global event listener (singleton) | N/A       | CGEventTap + subscriptions |
| `input_hook/`    | Global input hook (recording)| rdev           | Subscribes to events singleton |
| `mouse_tracker/` | Global mouse position        | rdev           | Subscribes to events singleton |
| `window/`        | Window API (`DWMWA_EXTENDED_FRAME_BOUNDS` rectangles on Windows) | Win32 API      | Quartz `CGWindowList` + `NSWorkspace` (frontmost app) |
| `pixel/`         | Pixel color reading          | GDI            | `CGWindowListCreateImage`, resolved with `dlsym` at first use (a missing symbol reads as "no color" instead of killing the process); `examples/pixel_probe.rs` compares it with ScreenCaptureKit |
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
