# Nebula -- Agent Guidelines

## Documentation management

Nebula is a project incubator -- each project is independent and self-contained. Each project owns its documentation:

| Location             | Purpose                                            |
| -------------------- | -------------------------------------------------- |
| `<project>/AGENTS.md`| Project-specific architecture and conventions      |
| `<project>/README.md`| Setup, build/run commands, prerequisites           |
| `<project>/specs/features/` | Evergreen feature specifications. Keep up-to-date when features change. |
| `<project>/specs/proposals/`| Point-in-time design proposals and RFCs. May be outdated -- verify against code. |

### Rules

- This file covers repo-wide conventions only. Project-specific architecture lives in each project's `AGENTS.md`.
- Do not create documentation files unless explicitly requested. Do not duplicate information across levels.

## Project directory

```
nebula/
  tap/              # Desktop automation tool (Tauri + React + Rust)
  wardenly-rs/      # Browser automation for WLY game (Tauri + React + Rust)
  wardenly-go/      # Browser automation for WLY game -- Go edition (Go + Fyne)
```

## Coding conventions

### Rust

- **Formatting**: `cargo fmt` (default rustfmt config)
- **Linting**: `cargo clippy -- -D warnings`
- **Architecture**: Layered (Domain → Application → Infrastructure → Adapter). Inner layers must not depend on outer layers.
- **Error handling**: Use typed errors (`thiserror`). Never silently swallow errors. Prefer `Result` propagation over panics.
- **Concurrency**: Prefer channels and message passing over shared mutable state. Use `Arc<Mutex<_>>` only when unavoidable.
- **Platform abstraction**: OS-specific code must be isolated behind traits or `cfg` gates in a dedicated platform module.

### TypeScript / React

- **Formatting**: Prettier
- **Linting**: ESLint with `typescript-eslint`
- **Styling**: Tailwind CSS. Use semantic CSS variables for theming -- never hard-code color values.
- **Components**: Functional components with hooks. Keep components small and single-purpose.
- **State management**: Zustand for global state. Avoid prop drilling beyond 2 levels.

### Go

- **Formatting**: `gofmt` + `golangci-lint`
- **Architecture**: Layered (Domain → Application → Infrastructure → Presentation). Same dependency direction as Rust projects.
- **Naming**: snake_case for file names, PascalCase for exported identifiers, camelCase for unexported.

### General

- Use descriptive, English-language variable and function names.
- snake_case for file names across all languages.
- Never commit secrets, API keys, or credentials. Use `.env` files (gitignored).

## Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/). Use scopes to indicate the affected project (e.g., `feat(tap): add timeline editor`).

Valid types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test`

## CI/CD

GitHub Actions for CI. Per-project release workflows triggered by project-scoped tags (e.g. `tap-v0.1.0`).
