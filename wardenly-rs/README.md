# Wardenly

A cross-platform desktop application for managing browser automation tasks for WLY game.

## Prerequisites

- **Rust** 1.75+
- **Node.js** 18+
- **Yarn** (package manager)
- **Chrome/Chromium** browser (for browser automation)
- Supported operating systems:
  - Windows 10+
  - macOS 10.15+
  - Linux (X11 or Wayland)

## Quick Start

```bash
# Install frontend dependencies
yarn install

# Run in development mode
yarn tauri dev
```

## Production Build

```bash
# Build optimized release
yarn tauri build
```

The built application will be in `src-tauri/target/release/`.

## Data Directory

All user data is stored in a platform-specific directory:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/wardenly/` |
| Windows | `%APPDATA%\wardenly\` |
| Linux | `~/.config/wardenly/` |

**Directory contents:**

| File/Directory | Description |
|----------------|-------------|
| `settings.yaml` | User settings (theme, storage backend) |
| `data.db` | SQLite database (local storage mode) |
| `logs/` | Application logs (release build only) |

## Configuration

### User Settings

Settings can be changed via the in-app Settings dialog.

```yaml
# settings.yaml
theme: ocean-dark
storage:
  type: sqlite  # or "mongodb"
  mongodb:
    uri: "mongodb://localhost:27017"
    database: "wardenly"
```

When using MongoDB:
- Connection is verified before saving
- On startup failure, the app falls back to SQLite with a warning

### Embedded Configs

Default configuration files shipped with the app, located in `src-tauri/resources/configs/`:

| File | Description |
|------|-------------|
| `themes.yaml` | Theme definitions (colors, fonts) |
| `keyboard.yaml` | Keyboard passthrough timing (long press threshold, repeat interval) |

## macOS Permissions

When using Keyboard Passthrough feature, grant Accessibility permission:  
**System Settings → Privacy & Security → Accessibility**

## Documentation

- [Functional Guide](docs/FUNCTIONAL_GUIDE.md) - Feature overview and usage
- [Project Structure](docs/PROJECT_STRUCTURE.md) - Architecture design
- [UI Design](docs/UI_DESIGN.md) - UI/UX specifications

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](../LICENSE) file for details.
