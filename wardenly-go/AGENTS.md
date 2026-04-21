# Wardenly (Go) -- Agent Guidelines

## Overview

Wardenly-go is a desktop application built with Go and Fyne for WLY browser game automation. It manages headless Chrome sessions via ChromeDP, provides real-time game view, and runs automated scripts based on scene recognition.

> **Note**: This is the original Go implementation. The Rust rewrite (`wardenly-rs`) is the active development line. This project is in maintenance mode.

## Architecture

### Layered design

```
┌──────────────────────────────────────────────────────┐
│  Presentation (Fyne UI)                               │
│  MainWindow, SessionList, SessionTab, CanvasWindow    │
├──────────────────────────────────────────────────────┤
│  Application (Business Logic)                         │
│  Coordinator, Session Actor, ScriptRunner             │
├──────────────────────────────────────────────────────┤
│  Domain (Models)                                      │
│  Account, Group, Scene, Script                        │
├──────────────────────────────────────────────────────┤
│  Infrastructure (External I/O)                        │
│  MongoDB, ChromeDP driver, OCR client, Logging        │
└──────────────────────────────────────────────────────┘
```

### Core patterns

- **Actor model**: Each Session runs as a goroutine with a serial command channel. Commands are processed one at a time.
- **Event-driven**: `EventBus` (async publish-subscribe) decouples components. Commands express user intent; events express state changes.
- **Command-Event separation**: UI sends Commands → Coordinator → Session; Session publishes Events → EventBus → UIEventBridge → UI update.

### Session state machine

```
Idle → Starting → LoggingIn → Ready ⇄ ScriptRunning
                                 │
                                 └──→ Stopped
```

### Directory structure

```
wardenly-go/
├── cmd/wardenly/main.go        # Entry point and dependency injection
├── core/                       # Core abstractions (Command, Event, State, EventBus)
├── domain/                     # Domain models (Account, Group, Scene, Script)
├── application/                # Session Actor, Coordinator, ScriptRunner
├── presentation/               # Fyne UI layer
├── infrastructure/             # MongoDB, ChromeDP, OCR, logging
├── resources/                  # Embedded resources (scenes, scripts, icons)
├── tools/                      # Dev tools (scene-analyzer, scene-generator)
├── specs/
│   ├── features/               # Feature specifications
│   └── proposals/              # Design proposals
└── README.md
```

### Key conventions

- **Logging**: Use `slog` with structured key-value pairs. Build-tag based environment selection (`prod` tag → file logging with lumberjack rotation; no tag → console).
- **Browser driver**: `Driver` interface in `infrastructure/browser/` allows swapping implementations. Current: ChromeDP with headless Chrome (viewport 1080×720).
- **Canvas management**: Fyne UI updates must happen on the main thread. `CanvasManager` uses a command queue to serialize frame updates and avoid race conditions.
- **Screencast delay**: 1-second delay after driver start before enabling frame sync, to avoid blank frames during browser initialization.

### Spec references

| Topic                    | Spec                                          |
|--------------------------|-----------------------------------------------|
| Features & usage         | `specs/features/functional-guide.md`          |
