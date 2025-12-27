# Wardenly

A desktop application built with Go and Fyne for managing and controlling browser automation tasks for WLY game.

## Project Structure

```
wardenly-go/
├── cmd/wardenly-go/main.go  # Application entry point
├── core/                    # Core abstractions (Command, Event, State, EventBus)
├── domain/                  # Domain models (Account, Scene, Script)
├── infrastructure/          # External integrations (MongoDB, ChromeDP, OCR)
├── application/             # Business logic (Session Actor, Coordinator)
├── presentation/            # UI layer (MainWindow, SessionTab, CanvasWindow)
├── resources/               # Embedded resources (scenes, scripts, icons)
├── docs/                    # Documentation
└── winres/                  # Windows resource configurations
```

## Prerequisites

- Go 1.23 or higher
- MongoDB (for account management)
- Chrome/Chromium browser (for ChromeDP)
- One of the following operating systems:
  - Windows 7 or later
  - macOS 10.13 or later
  - Linux with X11 or Wayland

## Key Dependencies

- [Fyne](https://fyne.io/) v2.5.2 - Cross-platform UI framework
- [ChromeDP](https://github.com/chromedp/chromedp) - Browser automation
- [MongoDB Go Driver](https://github.com/mongodb/mongo-go-driver) - Database operations

## Building the Application

### Using Build Script (Recommended)

```powershell
# Development build
.\build.ps1

# Production build (optimized, no console window)
.\build.ps1 -prod
```

### Manual Build

```powershell
# Install Windows resource tool
go install github.com/tc-hib/go-winres@latest

# Generate Windows resources
go-winres make

# Development build
go build -o wardenly-go.exe ./cmd/wardenly-go

# Production build (optimized, no console)
go build -trimpath -ldflags="-s -w -H windowsgui" -o wardenly-go.exe ./cmd/wardenly-go
```

## Running

```powershell
.\wardenly-go.exe
```

## Documentation

- [Project Structure](docs/PROJECT_STRUCTURE.md) - 项目架构设计
- [Functional Guide](docs/FUNCTIONAL_GUIDE.md) - 功能说明手册

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](../LICENSE) file for details.

## Acknowledgments

- Fyne team for the excellent UI framework
- ChromeDP team for the browser automation capabilities
- MongoDB team for the Go driver
