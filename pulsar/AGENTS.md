# Pulsar -- Agent Guidelines

## Overview

Pulsar 是面向开发者的**本地工具工作台**：把每天要用的几十个小工具（JSON 格式化、Base64、时间戳转换、JWT 解析、正则测试……）收进一个轻量、离线、跨平台的桌面 App。技术栈 Tauri v2 + React + Rust。

与赛道内既有产品（DevToys / IT-Tools 等）相比，Pulsar 的差异化在于：**纯离线轻量 + Smart Detection（粘贴即识别）+ Pipeline（工具串联）+ CLI/工作流自动化**，而非工具数量。详见 `specs/proposals/`。

> 当前阶段：Phase 1 进行中。Tool 内核、注册表、Smart Detection 雏形与 30 个 P0/P1 工具（纯文本/逻辑类）已落地；Pipeline、CLI、工作流、流式处理仍在路线图上。本文件描述**目标架构**；实现推进时需同步更新。

## Architecture

### 分层设计 (DDD + Onion)

依赖方向：外 → 内，内层永不依赖外层（与 `wardenly-rs` 一致）。下图为**目标架构**，`(规划)` 标记尚未实现的部分（实现进度见 `specs/proposals/roadmap.md`）。

```
┌──────────────────────────────────────────────────────────────┐
│                         Adapter 层                            │
│  src-tauri (GUI: Tauri IPC) ✅  ·  pulsar-cli (命令行) ✅     │
├──────────────────────────────────────────────────────────────┤
│                       Application 层 (pulsar-app)             │
│  ToolRegistry ✅ · Smart Detection ✅（注册表方法）           │
│  Pipeline (规划) · Workflow (规划) · ClipboardWatcher (规划)  │
├──────────────────────────────────────────────────────────────┤
│                  Infrastructure 层 (规划, 尚无此 crate)        │
│  剪贴板 · 文件流式 IO · 持久化 (SQLite) · 日志                 │
├──────────────────────────────────────────────────────────────┤
│                    Domain 层 (pulsar-core)                    │
│  Tool trait ✅ · ToolDescriptor ✅ · ToolValue(Text/Bytes) ✅ │
│  · 30 个工具纯实现 ✅                                          │
└──────────────────────────────────────────────────────────────┘
```

| Layer | Responsibility | May depend on |
|-------|----------------|---------------|
| **Domain** (`pulsar-core`) | 工具 trait、描述符、值类型、各工具纯实现（无 IO/UI/平台依赖） | 无 |
| **Application** (`pulsar-app`) | 注册表 + Smart Detection ✅；Pipeline / 工作流 / 剪贴板编排（规划） | Domain |
| **Infrastructure** | 剪贴板、文件 IO、持久化、日志（规划，尚未建 crate） | Domain |
| **Adapter** | `src-tauri`（GUI）✅ / `pulsar-cli`（命令行）✅ | Application |

> **实现现状**：`ToolValue` 目前只有 `Text` / `Bytes`（无 `Stream`，流式待大文件阶段）；Smart Detection 以 `ToolRegistry::detect()` 方法实现（非独立 `SmartDetector` 类型）；持久化目前仅前端收藏（localStorage），无 SQLite。

### 目录结构

```
pulsar/
├── src/                    # React 前端 (Vite)
│   ├── components/         # UI 组件 (sidebar, tool panel, layouts/, ui/, DetectBar, CommandPalette)
│   ├── stores/            # Zustand 状态 (registryStore, toolStore, uiStore)
│   ├── lib/               # ipc.ts (IPC 边界) / events.ts (事件接线) / layouts.ts (布局原型/示例/输入提示) / image.ts (SVG→PNG/剪贴板) / text.ts (行·字符度量)
│   └── styles/            # 全局样式 + 语义化 CSS 变量
├── src-tauri/             # Tauri 后端 (GUI 适配，薄；插件：clipboard-manager 用于复制图片)
├── crates/
│   ├── pulsar-core/       # 纯域：Tool trait、描述符、ToolValue、30 个工具实现、detect 规则
│   ├── pulsar-app/        # 应用层：注册表 + Smart Detection（detect 方法）
│   └── pulsar-cli/        # CLI 适配（bin `pulsar`）：descriptor→clap 动态派生，dispatch.rs 是唯一映射层
│   # Pipeline / Workflow / Infrastructure 仍为规划
├── specs/
│   ├── features/          # 功能规格（落地后维护）
│   └── proposals/         # 设计提案 (vision / architecture / roadmap)
└── README.md
```

### 核心约定

- **一份逻辑多处共享**：每个工具是纯函数 `fn(ToolValue, &ToolParams) -> Result<ToolValue>`，全部住在 `pulsar-core`。GUI 与 CLI 调用同一份逻辑、**零重复**；Pipeline 为规划中的第三种形态。
- **加工具只改一处**：新增工具 = 实现 `Tool` trait + 在 `pulsar-app/src/registry.rs` 的 `build_registry()` 注册一条 `ToolDescriptor`（并更新计数断言）。UI 表单、**CLI 子命令/flag**、Smart Detection 候选**全部由 descriptor 派生**——GUI 与 CLI 自动同步，无需手工改两处（CLI 的派生逻辑集中在 `pulsar-cli/src/dispatch.rs`）。
- **工具 id 规范**：`<category>.<tool>`，如 `encoders.base64`、`converters.json_yaml`。分类 enum：`Converters / Encoders / Formatters / Generators / Testers / Text / Graphic / Reference`。
- **纯函数内核**：工具逻辑无副作用，文件/剪贴板由外层喂入。`ToolValue` 现为 `Text` / `Bytes`；大文件流式（`Stream`）为规划项，避免全量载入内存。
- **离线优先**：不做需要联网的工具，数据不出本机。
- **ID 方案**：ULID（时间有序），与仓库统一。

### 前端架构 (`src/`)

store-driven，组件保持薄，读写 Zustand store：

| Store | 职责 |
|-------|------|
| `registryStore` | 工具列表/分类/参数 schema（从后端拉取）、搜索、收藏（localStorage 持久化） |
| `toolStore` | 当前工具输入/输出/参数、运行状态；按工具 id 暂存会话（切页恢复，见下） |
| `uiStore` | 命令面板开关（Cmd/Ctrl+K）等全局界面状态 |

> 规划中：`pipelineStore`（Pipeline 步骤编辑与执行）、"最近使用"、Compact 浮窗等尚未实现。

- **切页暂存（per-tool 会话）**：`toolStore.sessions` 按工具 id 暂存 `{input, params, output, error}`。`selectTool` 切走前存当前、切回时恢复"离开时的样子"；点同一工具不重置。**仅内存**，刷新/重启即清空（不持久化，符合预期）。Smart Detection 跳转（`selectAndFill`）会用识别内容覆盖该工具会话。

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

### CLI 架构 (`crates/pulsar-cli`)

二进制名 `pulsar`，是与 GUI 并列的薄适配层——只做"解析参数 → 调注册表 → 打印结果"，**不含任何按工具写死的代码**。

- **单一事实源**：子命令、flag、帮助文本全部由 `ToolRegistry` 的 `ToolDescriptor` **动态派生**。新增工具或改参数只动 `pulsar-core` + 注册一行，CLI 自动同步（与 GUI 表单同源）。
- **唯一映射层**：descriptor → clap 的转换全部集中在 `dispatch.rs`（命令名、flag、`ToolParams` 收集），约定清晰：
  - 命令名 = id 短名（`encoders.base64` → `base64`）；撞名或撞保留字（`list`/`detect`/`help`）退回完整 id。完整 id 始终注册为可见别名。
  - `Bool` → `--key` / `--no-key`（`--no-` 优先）；`Int`/`Str` → `--key <值>`；`Enum` → `--key <候选>`（带校验）。
  - **只收集用户显式给的参数**，其余留空交由工具自身默认（默认值的唯一真理在工具里，不在 CLI 复制）。
- **I/O 契约**（脚本/CI/agent 友好）：主输入走 stdin（管道）或位置参数；stdin 为 TTY 且无位置参数时视为空输入（避免生成类工具卡住）。结果→stdout；错误→stderr + 非零退出码。
- **内置子命令**：`list`/`detect`（均支持 `--json`）、`completions <shell>`（用同一棵动态命令树经 `clap_complete` 渲染补全脚本，工具自动覆盖）。三者与 `help` 同为保留字，工具撞名退回完整 id。
- clap 4 要求命令/参数名 `'static`，而名字是运行期派生的：`dispatch.rs::leak` 在启动时 leak 少量短字符串（命令树随进程存活，无实际泄漏风险）。
- 测试：`dispatch.rs` 内有映射单测；`tests/cli.rs` 用 `assert_cmd` 跑真实二进制验证 I/O 与退出码。

### 关键能力（差异化）

| 能力 | 说明 | 状态 |
|------|------|------|
| Smart Detection | 粘贴内容自动识别类型并推荐工具（descriptor 声明 detectors，`registry.detect()` + 前端 DetectBar） | ✅ Phase 1 |
| Command Palette | `Cmd/Ctrl+K` 模糊搜全部工具 | ✅ Phase 1 |
| Pipeline | 工具串联，前一步输出喂后一步，构建期类型校验 | ⏳ Phase 2（未实现） |
| CLI | `cat in.json \| pulsar json`，子命令由注册表动态派生，可进 CI/脚本 | ✅ Phase 3 |
| 工作流 + 剪贴板监听 | 保存 Pipeline 复用；监听剪贴板自动处理（接续 tap 基因） | ⏳ Phase 3（未实现） |
| 流式大文件 | 数百 MB JSON/日志/CSV 不爆内存 | ⏳ Phase 3（未实现，`ToolValue::Stream` 未引入） |

## Spec references

| Topic | Spec |
|-------|------|
| 愿景 / 定位 / 工具目录 / IA | `specs/proposals/vision-and-scope.md` |
| 架构设计 | `specs/proposals/architecture.md` |
| 开发路线图 | `specs/proposals/roadmap.md` |
