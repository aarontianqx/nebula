# Wardenly (Rust) -- Agent Guidelines

## Overview

Wardenly is a cross-platform desktop application for WLY browser game automation, built with Tauri v2 + React + Rust. It manages headless browser sessions, provides real-time game view, and runs automated scripts based on scene recognition.

## Architecture

### Layered design (DDD + Onion)

Dependency direction: outer → inner. Inner layers never depend on outer layers.

```
┌─────────────────────────────────────────────────────────────┐
│                       Adapter 层                             │
│  Tauri IPC commands, frontend event emission                 │
├─────────────────────────────────────────────────────────────┤
│                     Application 层                           │
│  Coordinator, SessionActor, ScriptRunner, EventBus           │
├─────────────────────────────────────────────────────────────┤
│                    Infrastructure 层                          │
│  SQLite/MongoDB, chromiumoxide, OCR client, config loader     │
├─────────────────────────────────────────────────────────────┤
│                       Domain 层                              │
│  Account, Group, Session, Scene, Script, Expression          │
└─────────────────────────────────────────────────────────────┘
```

| Layer              | Responsibility                                      | May depend on    |
|--------------------|-----------------------------------------------------|------------------|
| **Domain**         | Entities, value objects, repository traits, events   | Nothing          |
| **Application**    | Use-case orchestration, command handling, event bus  | Domain           |
| **Infrastructure** | Database, browser driver, keyboard, config           | Domain           |
| **Adapter**        | Tauri IPC, frontend communication                    | Application      |

### Core patterns

- **Actor model**: Each Session runs as an independent actor with a serial command channel (`mpsc`). This eliminates data races on session state.
- **Event-driven**: Commands represent user intent; Events represent state changes. The `EventBus` (`tokio::sync::broadcast`) decouples publishers and subscribers.
- **Platform abstraction**: Platform-specific code (keyboard listener) is isolated behind traits.

### Session state machine

```
Idle → Starting → LoggingIn → Ready ⇄ ScriptRunning
                                 │
                                 └──→ Stopped
```

### Directory structure

```
wardenly-rs/
├── src/                            # React frontend
│   ├── components/                 # UI components (layout, session, canvas, dialogs, forms)
│   ├── providers/                  # ThemeProvider (runtime CSS variable injection)
│   ├── hooks/                      # Tauri event listeners
│   ├── stores/                     # Zustand state (account, session)
│   ├── types/                      # TypeScript type definitions
│   └── styles/                     # Global styles + CSS variables
│
├── src-tauri/src/                  # Rust backend
│   ├── domain/                     # Domain layer (model, repository traits, events, errors)
│   ├── application/                # Application layer (coordinator, session actor, script runner)
│   ├── infrastructure/             # Infrastructure (persistence, browser, config, OCR, logging)
│   └── adapter/tauri/              # Adapter layer (Tauri commands, events, state)
│
├── src-tauri/resources/            # Embedded resources (read-only)
│   ├── configs/                    # themes.yaml, keyboard.yaml
│   ├── scenes/                     # Scene definitions (*.yaml)
│   └── scripts/                    # Script definitions (*.yaml)
│
├── specs/
│   ├── features/                   # Feature specifications
│   └── proposals/                  # Design proposals and RFCs
└── README.md
```

### Key conventions

- **Storage runtime switch**: Repository uses trait objects (`dyn AccountRepository`) for runtime polymorphism between SQLite and MongoDB. No compile-time feature flags needed.
- **Theme system**: Theme presets are defined in `themes.yaml` (embedded resource). At runtime, ThemeProvider reads via Tauri command and injects CSS variables into `:root`. No recompilation for theme changes.
- **ScriptRunner lifecycle**: Each script run gets a unique `run_id`. Stop commands carry optional `run_id` for race-condition safety. A shared `running` flag allows immediate stop marking.
- **Protocol bridge**: `resources/page_bridge.js` is injected via CDP init script before game boot. It patches the game's own `Connection` to report all downstream packets as `ProtocolMessage` events; upstream sends go through `SessionCommand::SendProtocol` → `window.__wardenly.send(name, payload)`. No binary/crypto is touched — the game encodes and decodes itself.
- **Two script engines**: scene scripts (`resources/scripts/`, scene-recognition loop via `ScriptRunner`) and protocol scripts (`resources/protocols/`, linear protocol exchange via `ProtocolRunner`). `start_script` picks the engine by name lookup. Protocol script conditions query the per-session `GameState` (latest payload per protocol name, fed by the bridge forwarder) instead of OCR. Protocol names are validated at start against `resources/protocols/registry.json`.
- **Exact-state conditions**: shared `condition_eval` resolves `state.*` (GameState) and `role.*` (live client role model via the bridge's `queryRole`) paths, including `$`-prefixed field references. Used by protocol script conditions and by scene steps' `stateRule` — the OCR-free replacement for `ocrRule` (same decision points, same quit/skip actions).
- **Login ready criterion**: `Connection._connected` AND bridge-observed `S_2_C_CHAR_LOAD_END` — the WS connects early, possibly while a canvas dialog still blocks game entry, so observed login traffic is the only reliable "server accepts business protocols" signal. A viewport self-check after login warns if the drawing area deviates from the 1080x720 all scene/script coordinates assume.
- **Humanization (cross-cutting, never task-specific)**: input layer jitters click/drag endpoints (±2px) and randomizes hold time (30–90ms) in `ChromiumDriver`; pacing layer (`service/humanize.rs`) adds a randomized 150–450ms delay before every script action in both runners and randomizes staggered-start intervals (0.5–2.0s). No YAML changes — applies to all current and future scripts uniformly.
- **OCR**: Global singleton `HttpOcrClient` with background health checks. OCR rules use expression-based conditions (`"used > 7 || used > total"`).
- **ID scheme**: ULID for all entity IDs (time-ordered unique identifiers).

### Spec references

| Topic                    | Spec                                          |
|--------------------------|-----------------------------------------------|
| Features & usage         | `specs/features/functional-guide.md`          |
| UI/UX design             | `specs/features/ui-design.md`                 |
