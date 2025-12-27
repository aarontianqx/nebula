# Wardenly

A cross-platform desktop application built with Rust and Tauri for managing and controlling browser automation tasks for WLY game.

## Project Structure

```
wardenly-rs/
├── src-tauri/               # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs          # Application entry point
│   │   ├── lib.rs           # Library entry point
│   │   ├── domain/          # Domain models (Account, Group, Scene, Script)
│   │   ├── application/     # Business logic (Services, Session Actor)
│   │   ├── infrastructure/  # External integrations (SQLite, Config, Logging)
│   │   └── adapter/         # Interface adapters (Tauri commands)
│   ├── resources/           # Embedded resources (configs)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                     # Frontend (React + TypeScript)
│   ├── App.tsx
│   ├── main.tsx
│   ├── components/          # UI components
│   └── stores/              # State management (Zustand)
└── docs/                    # Documentation
```

## Prerequisites

- Rust 1.75 or higher
- Node.js 18 or higher (for frontend build)
- Yarn (package manager)
- One of the following operating systems:
  - Windows 10 or later
  - macOS 10.15 or later
  - Linux with X11 or Wayland

### Optional (Phase 2+)

- Chrome/Chromium browser (for browser automation)
- MongoDB (if using MongoDB storage backend)

## Key Dependencies

### Backend (Rust)

- [Tauri](https://tauri.app/) v2 - Cross-platform desktop framework
- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite database
- [tokio](https://tokio.rs/) - Async runtime
- [tracing](https://tracing.rs/) - Structured logging
- [serde](https://serde.rs/) - Serialization/Deserialization

### Frontend

- [React](https://react.dev/) 18 - UI library
- [TypeScript](https://www.typescriptlang.org/) - Type-safe JavaScript
- [Tailwind CSS](https://tailwindcss.com/) v4 - Utility-first CSS
- [Zustand](https://zustand.docs.pmnd.rs/) - State management
- [Lucide React](https://lucide.dev/) - Icons

## Building the Application

### Development

```bash
# Install frontend dependencies
yarn install

# Run in development mode (hot-reload enabled)
yarn tauri dev
```

### Production Build

```bash
# Build optimized release
yarn tauri build
```

The built application will be in `src-tauri/target/release/`.

## Running

```bash
# Development
yarn tauri dev

# Or run the built binary directly
./src-tauri/target/release/wardenly-rs
```

## Configuration

Application configuration is in `src-tauri/resources/configs/app.yaml`:

```yaml
storage:
  sqlite:
    # Leave empty for platform default path:
    # - macOS: ~/Library/Application Support/wardenly/data.db
    # - Windows: %APPDATA%/wardenly/data.db
    # - Linux: ~/.config/wardenly/data.db
    path: ""
```

## Documentation

- [Project Structure](docs/PROJECT_STRUCTURE.md) - 项目架构设计
- [Functional Guide](docs/FUNCTIONAL_GUIDE.md) - 功能说明手册
- [UI Design](docs/UI_DESIGN.md) - UI 设计规范
- [Roadmap](docs/roadmap/ROADMAP.md) - 开发路线图

## Development Roadmap

- **Phase 1** ✅ - Core Framework (Account/Group CRUD, SQLite, Config)
- **Phase 2** ✅ - Browser Integration (chromiumoxide, Session Actor, Canvas)
- **Phase 3** - Script Engine (Scene matching, Script execution, OCR)
- **Phase 4** - Extensibility (Keyboard passthrough, Batch operations, MongoDB)

## License

This project is licensed under the MIT License - see the LICENSE file for details.
