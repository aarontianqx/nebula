# tap (Timed Action Performer)

`tap` 是一个桌面自动化应用（GUI），目标是处理大量重复性操作：简单重复点击、操作录制/重放，以及后续的可编程/可配置/插件式扩展。

技术栈：Tauri + React + Rust

## 快速开始

### 开发

```bash
cd tap
npm install
npm run tauri:dev
```

### 构建（发布 / 打包）

```bash
cd tap
npm install
npm run tauri:build
```

## 安全停止

**全局热键：`Ctrl + Shift + Backspace`** — 随时立即停止执行

## 文档

- `docs/FUNCTIONAL_GUIDE.md` — 产品功能
- `docs/PROJECT_STRUCTURE.md` — 技术架构
- `docs/UI_DESIGN.md` — UI/UX 设计
- `docs/DSL_REFERENCE.md` — YAML DSL 语法参考
- `docs/roadmap/` — 开发路线图
- `templates/` — YAML 宏模板示例
