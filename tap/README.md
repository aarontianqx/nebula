# tap (Timed Action Performer)

`tap` 是一个桌面自动化应用（GUI），目标是处理大量重复性操作：简单重复点击、操作录制/重放，以及后续的可编程/可配置/插件式扩展。

技术栈：Tauri + React + Rust

## 快速开始

### 开发

```bash
cd tap
yarn install
yarn tauri:dev
```

### 代码检查（提交前 / CI）

```bash
# 前端：类型检查 + Lint
yarn typecheck
yarn lint

# Rust：格式化 + Clippy + 测试
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p tap-core -p tap-application -p tap-platform -p tap-tauri
```

### 构建（发布 / 打包）

```bash
cd tap
yarn install
yarn tauri:build
```

## 安全停止

**全局热键：`Ctrl + Shift + Backspace`** — 随时立即停止执行

## 文档

- `AGENTS.md` — 架构设计与开发规范
- `specs/features/` — 功能规格（产品功能、UI 设计、DSL 语法参考等）
- `specs/proposals/` — 设计提案与路线图
- `templates/` — YAML 宏模板示例

## License

This project is licensed under the GNU Affero General Public License v3.0 - see the [LICENSE](../LICENSE) file for details.
