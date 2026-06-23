# Pulsar -- Agent Guidelines

## Overview

Pulsar 是面向开发者的**本地工具工作台**：把每天要用的几十个小工具（JSON 格式化、Base64、时间戳转换、JWT 解析、正则测试……）收进一个轻量、离线、跨平台的桌面 App。技术栈 Tauri v2 + React + Rust。

与赛道内既有产品（DevToys / IT-Tools 等）相比，Pulsar 的差异化在于：**纯离线轻量 + Smart Detection（粘贴即识别）+ Pipeline（工具串联）+ CLI/工作流自动化**，而非工具数量。详见 `specs/proposals/`。

> 当前阶段：Phase 1 进行中。Tool 内核、注册表、Smart Detection 雏形与 25 个 P0/P1 工具（纯文本/逻辑类）已落地；Pipeline、CLI、工作流、流式处理仍在路线图上。本文件描述**目标架构**；实现推进时需同步更新。

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
│   ├── lib/               # ipc.ts (IPC 边界) / events.ts (事件接线)
│   └── styles/            # 全局样式 + 语义化 CSS 变量
├── src-tauri/             # Tauri 后端 (GUI 适配，薄)
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
| `registryStore` | 工具列表/分类/参数 schema（从后端拉取） |
| `toolStore` | 当前工具输入/输出/参数、运行状态 |
| `pipelineStore` | Pipeline 步骤编辑与执行（V2） |
| `uiStore` | 视图模式、搜索、收藏、最近使用、Compact 浮窗 |

- **IPC 边界**：所有后端调用走 `lib/ipc.ts`（带 guard，无 Tauri 运行时也能渲染）；事件在 `lib/events.ts` 一处接线。
- **样式**：Tailwind + 语义化 CSS 变量，禁止硬编码颜色。

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
