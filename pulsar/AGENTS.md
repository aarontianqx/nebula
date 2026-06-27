# Pulsar -- Agent Guidelines

## Overview

Pulsar 是面向开发者的**本地工具工作台**：把每天要用的几十个小工具（JSON 格式化、Base64、时间戳转换、JWT 解析、正则测试……）收进一个轻量、离线、跨平台的桌面 App。技术栈 Tauri v2 + React + Rust。

与赛道内既有产品（DevToys / IT-Tools 等）相比，Pulsar 的差异化在于：**纯离线轻量 + Smart Detection（粘贴即识别）+ Pipeline（工具串联）+ CLI/工作流自动化**，而非工具数量。详见 `specs/proposals/`。

> 当前阶段：Phase 1 进行中。Tool 内核、注册表、Smart Detection 雏形与 30 个 P0/P1 工具（纯文本/逻辑类）已落地；Pipeline、CLI、工作流、流式处理仍在路线图上。本文件描述**目标架构**；实现推进时需同步更新。

## Architecture

### 分层设计 (DDD + Onion)

依赖方向：外 → 内，内层永不依赖外层（与 `wardenly-rs` 一致）。

```
┌──────────────────────────────────────────────────────────────┐
│                         Adapter 层                            │
│  src-tauri (GUI: Tauri IPC)   ·   pulsar-cli (命令行)         │
├──────────────────────────────────────────────────────────────┤
│                       Application 层 (pulsar-app)             │
│  ToolRegistry · Pipeline · SmartDetector · Workflow ·         │
│  ClipboardWatcher                                             │
├──────────────────────────────────────────────────────────────┤
│                     Infrastructure 层                          │
│  剪贴板 · 文件流式 IO · 持久化 (SQLite) · 日志                 │
├──────────────────────────────────────────────────────────────┤
│                    Domain 层 (pulsar-core)                    │
│  Tool trait · ToolDescriptor · ToolValue · 各工具纯实现        │
└──────────────────────────────────────────────────────────────┘
```

| Layer | Responsibility | May depend on |
|-------|----------------|---------------|
| **Domain** (`pulsar-core`) | 工具 trait、描述符、值类型、各工具纯实现（无 IO/UI/平台依赖） | 无 |
| **Application** (`pulsar-app`) | 注册表、Pipeline、Smart Detection、工作流、剪贴板编排 | Domain |
| **Infrastructure** | 剪贴板、文件 IO、持久化、日志 | Domain |
| **Adapter** | `src-tauri`（GUI）/ `pulsar-cli`（命令行） | Application |

### 目录结构

```
pulsar/
├── src/                    # React 前端 (Vite)
│   ├── components/         # UI 组件 (layout, tool panel, pipeline, dialogs)
│   ├── stores/            # Zustand 状态 (registry, tool, pipeline, ui)
│   ├── lib/               # ipc.ts (IPC 边界) / events.ts (事件接线) / layouts.ts (布局原型/示例/输入提示) / image.ts (SVG→PNG/剪贴板) / text.ts (行·字符度量)
│   └── styles/            # 全局样式 + 语义化 CSS 变量
├── src-tauri/             # Tauri 后端 (GUI 适配，薄；插件：clipboard-manager 用于复制图片)
├── crates/
│   ├── pulsar-core/       # 纯域：Tool trait、描述符、所有工具实现
│   ├── pulsar-app/        # 应用层：注册表、Pipeline、SmartDetector、Workflow
│   └── pulsar-cli/        # CLI 适配 (二进制)
├── specs/
│   ├── features/          # 功能规格（落地后维护）
│   └── proposals/         # 设计提案 (vision / architecture / roadmap)
└── README.md
```

### 核心约定

- **一份逻辑三处共享**：每个工具是纯函数 `fn(ToolValue, &ToolParams) -> Result<ToolValue>`，全部住在 `pulsar-core`。GUI、CLI、Pipeline 三种形态调用同一份逻辑，**零重复**。
- **加工具只改一处**：新增工具 = 实现 `Tool` trait + 在注册表注册一条 `ToolDescriptor`。UI 表单、CLI 子命令、Pipeline 兼容校验、Smart Detection 候选**全部由 descriptor 派生**。
- **工具 id 规范**：`<category>.<tool>`，如 `encoders.base64`、`converters.json_yaml`。分类 enum：`Converters / Encoders / Formatters / Generators / Testers / Text / Graphic / Reference`。
- **纯函数内核**：工具逻辑无副作用，文件/剪贴板由 Application 层喂入；大文件用 `ToolValue::Stream` 流式处理，避免全量载入内存。
- **离线优先**：不做需要联网的工具，数据不出本机。
- **ID 方案**：ULID（时间有序），与仓库统一。

### 前端架构 (`src/`)

store-driven，组件保持薄，读写 Zustand store：

| Store | 职责 |
|-------|------|
| `registryStore` | 工具列表/分类/参数 schema（从后端拉取）、收藏（localStorage 持久化） |
| `toolStore` | 当前工具输入/输出/参数、运行状态 |
| `pipelineStore` | Pipeline 步骤编辑与执行（V2） |
| `uiStore` | 命令面板开关（Cmd/Ctrl+K）等全局界面状态；后续承载视图模式 / Compact 浮窗 |

- **布局原型（archetype）**：不同工具的输入/输出形态差异很大，强行「双大文本框」并不通用。前端按工具 id 归到 5 种 archetype（见 `lib/layouts.ts`），由 `ToolPanel` 分发到 `components/layouts/` 下对应布局：
  - `transform`：大文本输入 → 大文本输出（编码 / 格式化 / 批量文本）
  - `inspect`：紧凑输入 → 结构化字段卡（时间戳 / 进制 / 颜色 / JWT / 哈希）
  - `generate`：无输入，纯参数表单 → 结果（密码 / ID）。结果走 `GenerateResult`：突出「主结果」（首个空行前的内容），**复制只取主结果**，元信息（长度 / 熵等）弱化为辅助说明，避免把元信息一起粘出去。
  - `query`：查询字段 + 主体文本 → 匹配（正则 / JSONPath / Diff）
  - `visual`：渲染输出（二维码）。走 `QrResult`：SVG 即时渲染，并支持「复制图片 / 下载 PNG」（可选 256/512/1024 分辨率，canvas 光栅化）。复制图片优先用 Tauri clipboard 插件写系统剪贴板（IM 可直接粘贴），退化到浏览器 `ClipboardItem`，再退化为下载。
  - 通用输出渲染走 `OutputView`：自动识别 SVG / 颜色 / 分段（`--- Title ---`）/ `LABEL: value` 字段，并提供「渲染 / 源码」切换；纯文本输出附带「行 · 字符」徽标与「自动换行」开关（关闭后长行横向滚动，便于看 SQL/JSON）。
  - 暂以前端 id 映射为单一来源，设计稳定后可上提到 Rust `ToolDescriptor`。
- **一键示例与输入提示**：`lib/layouts.ts` 的 `EXAMPLES` / `INPUT_HINTS` 为多数工具提供「示例」按钮与贴合内容的占位提示；示例须在**默认参数**下即产出有意义结果。`transform`/`inspect`/`query` 布局在标题栏渲染「示例」按钮（仅当该工具配了示例）。
- **轻量工具自动运行**：`lib/layouts.ts` 标注的工具输入即防抖运行（无需点「运行」）；`runsOnEmpty` 标注的工具（如时间戳，留空 = 当前时间）空输入也会运行，选中即出结果。全局 `Cmd/Ctrl+Enter` 运行。
- **命令面板**：全局 `Cmd/Ctrl+K` 打开 `CommandPalette`，按名称 / 关键词 / 分类模糊搜索全部工具，方向键 + 回车跳转。
- **工具描述**：标题旁信息图标（hover/focus 显示），不再常驻整行描述，保持页面简洁。
- **IPC 边界**：所有后端调用走 `lib/ipc.ts`（带 guard，无 Tauri 运行时也能渲染）；事件在 `lib/events.ts` 一处接线。
- **样式**：Tailwind + 语义化 CSS 变量（深色为主，`globals.css` 预留浅色 `data-theme`），禁止硬编码颜色。共享 UI 原子件在 `components/ui/`。

### 关键能力（差异化）

| 能力 | 说明 | 阶段 |
|------|------|------|
| Smart Detection ✅ | 粘贴内容自动识别类型并推荐工具（descriptor 声明 detectors，`registry.detect()`） | Phase 1（已实现雏形） |
| Command Palette | `Cmd/Ctrl+K` 模糊搜全部工具 | Phase 1 |
| Pipeline | 工具串联，前一步输出喂后一步，构建期类型校验 | Phase 2 |
| CLI | `pulsar json fmt < in.json`，可进 CI/脚本 | Phase 3 |
| 工作流 + 剪贴板监听 | 保存 Pipeline 复用；监听剪贴板自动处理（接续 tap 基因） | Phase 3 |
| 流式大文件 | 数百 MB JSON/日志/CSV 不爆内存 | Phase 3 |

## Spec references

| Topic | Spec |
|-------|------|
| 愿景 / 定位 / 工具目录 / IA | `specs/proposals/vision-and-scope.md` |
| 架构设计 | `specs/proposals/architecture.md` |
| 开发路线图 | `specs/proposals/roadmap.md` |
