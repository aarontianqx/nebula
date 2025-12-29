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

## Configuration

### User Settings

User settings are stored in `settings.yaml` at the platform-specific config directory:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/wardenly/settings.yaml` |
| Windows | `%APPDATA%\wardenly\settings.yaml` |
| Linux | `~/.config/wardenly/settings.yaml` |

Example configuration:

```yaml
theme: ocean-dark
storage:
  type: sqlite  # or "mongodb"
  mongodb:
    uri: "mongodb://localhost:27017"
    database: "wardenly"
```

Settings can be changed via the in-app Settings dialog. When using MongoDB:
- Connection is verified before saving
- On startup failure, the app falls back to SQLite with a warning

**Default data paths (SQLite):**

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/wardenly/data.db` |
| Windows | `%APPDATA%\wardenly\data.db` |
| Linux | `~/.config/wardenly/data.db` |

### Embedded Configs

Embedded configuration files in `src-tauri/resources/configs/`:

- `themes.yaml` - Theme definitions
- `gesture.yaml` - Keyboard passthrough settings

```yaml
# gesture.yaml
keyboard_passthrough:
  long_press_threshold_ms: 300
  repeat_interval_ms: 100
  debounce_window_ms: 50
```


## macOS Permissions

When using Keyboard Passthrough feature, grant Accessibility permission:  
**System Settings → Privacy & Security → Accessibility**

## Documentation

- [Functional Guide](docs/FUNCTIONAL_GUIDE.md) - Feature overview and usage
- [Project Structure](docs/PROJECT_STRUCTURE.md) - Architecture design
- [UI Design](docs/UI_DESIGN.md) - UI/UX specifications

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](../LICENSE) file for details.
