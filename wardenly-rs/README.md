# Wardenly

A cross-platform desktop application built with Rust and Tauri for managing and controlling browser automation tasks for WLY game.

## Project Structure

```
wardenly-rs/
├── src-tauri/               # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs          # Application entry point
│   │   ├── core/            # Core abstractions (Command, Event, State, EventBus)
│   │   ├── domain/          # Domain models (Account, Scene, Script)
│   │   ├── application/     # Business logic (Session Actor, Coordinator)
│   │   └── infrastructure/  # External integrations (SQLite/MongoDB, Browser, OCR)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                     # Frontend (React + TypeScript)
│   ├── App.tsx
│   ├── components/          # UI components
│   ├── hooks/               # Custom React hooks
│   └── stores/              # State management
├── resources/               # Embedded resources (scenes, scripts, icons)
└── docs/                    # Documentation
```

## Prerequisites

- Rust 1.75 or higher
- Node.js 18 or higher (for frontend build)
- Chrome/Chromium browser (for browser automation)
- One of the following operating systems:
  - Windows 10 or later
  - macOS 10.15 or later
  - Linux with X11 or Wayland

### Optional

- MongoDB (if using MongoDB storage backend)

## Key Dependencies

### Backend (Rust)

- [Tauri](https://tauri.app/) v2 - Cross-platform desktop framework
- [chromiumoxide](https://github.com/mattsse/chromiumoxide) - Browser automation via CDP
- [tokio](https://tokio.rs/) - Async runtime
- [SeaORM](https://www.sea-ql.org/SeaORM/) - Database ORM (SQLite/MongoDB)
- [tracing](https://tracing.rs/) - Structured logging

### Frontend

- [React](https://react.dev/) 18 - UI library
- [TypeScript](https://www.typescriptlang.org/) - Type-safe JavaScript
- [Tailwind CSS](https://tailwindcss.com/) - Utility-first CSS

## Building the Application

### Development

```bash
# Install frontend dependencies
npm install

# Run in development mode (hot-reload enabled)
npm run tauri dev
```

### Production Build

```bash
# Build optimized release
npm run tauri build
```

The built application will be in `src-tauri/target/release/`.

## Running

```bash
# Development
npm run tauri dev

# Or run the built binary directly
./src-tauri/target/release/wardenly
```

## Configuration

Storage backend can be configured in `config.toml`:

```toml
[storage]
# Options: "sqlite" (default) or "mongodb"
backend = "sqlite"

# SQLite settings (when backend = "sqlite")
sqlite_path = "~/.config/wardenly/data.db"

# MongoDB settings (when backend = "mongodb")
mongodb_uri = "mongodb://localhost:27017"
mongodb_database = "wardenly"
```

## Documentation

- [Project Structure](docs/PROJECT_STRUCTURE.md) - 项目架构设计
- [Functional Guide](docs/FUNCTIONAL_GUIDE.md) - 功能说明手册
- [UI Design](docs/UI_DESIGN.md) - UI 设计规范

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Tauri team for the excellent cross-platform framework
- chromiumoxide team for the Rust CDP implementation
- tokio team for the async runtime

