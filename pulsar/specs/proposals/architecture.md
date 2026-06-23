# Pulsar — 架构设计

> 状态：提案 (Proposal) · 起草于 2026-06
> 配套文档：[`vision-and-scope.md`](./vision-and-scope.md)、[`roadmap.md`](./roadmap.md)

## 1. 设计目标

1. **一份工具逻辑，三种形态共享**：GUI、CLI、Pipeline 调用同一份纯逻辑，零重复。
2. **加工具只改一处**：新增工具 = 写一个纯函数 + 注册一条元数据，UI/CLI 自动呈现。
3. **纯函数内核**：工具逻辑无 IO、无 UI、无平台依赖，易测试、可流式。
4. **遵循仓库分层规范**：Domain → Application → Infrastructure → Adapter，依赖单向向内。

## 2. 分层架构 (DDD + Onion)

依赖方向：外 → 内，内层永不依赖外层（与 `wardenly-rs` 一致）。

```
┌──────────────────────────────────────────────────────────────┐
│                         Adapter 层                            │
│  ┌────────────────────────┐   ┌───────────────────────────┐  │
│  │ src-tauri (GUI 适配)    │   │ pulsar-cli (命令行适配)    │  │
│  │ Tauri IPC commands      │   │ clap 解析 + stdin/stdout   │  │
│  └────────────────────────┘   └───────────────────────────┘  │
├──────────────────────────────────────────────────────────────┤
│                       Application 层                          │
│  ToolRegistry · Pipeline 执行器 · SmartDetector ·             │
│  WorkflowStore · ClipboardWatcher（监听自动化）               │
├──────────────────────────────────────────────────────────────┤
│                     Infrastructure 层                          │
│  剪贴板 · 文件流式 IO · 配置/历史持久化 (SQLite) · 日志        │
├──────────────────────────────────────────────────────────────┤
│                         Domain 层 (pulsar-core)               │
│  Tool trait · ToolDescriptor · ToolValue · 各工具纯实现        │
└──────────────────────────────────────────────────────────────┘
```

| 层 | 职责 | 可依赖 |
|----|------|--------|
| **Domain** (`pulsar-core`) | 工具 trait、描述符、值类型、各工具纯实现 | 无 |
| **Application** (`pulsar-app`) | 注册表、Pipeline、Smart Detection、工作流、剪贴板编排 | Domain |
| **Infrastructure** | 剪贴板、文件 IO、持久化、日志 | Domain |
| **Adapter** | `src-tauri`（GUI）/ `pulsar-cli`（命令行） | Application |

## 3. Crate 划分

```
pulsar/
├── crates/
│   ├── pulsar-core/      # 纯域：Tool trait、ToolDescriptor、ToolValue、所有工具实现
│   ├── pulsar-app/       # 应用层：注册表、Pipeline、SmartDetector、Workflow、ClipboardWatcher
│   └── pulsar-cli/       # CLI 适配（二进制，依赖 core + app）
├── src-tauri/            # GUI 适配（Tauri 后端，依赖 core + app）
├── src/                  # React 前端
├── specs/
│   ├── features/
│   └── proposals/
└── README.md
```

> `pulsar-core` 与 `pulsar-app` 不依赖 Tauri，可独立单测与基准测试。GUI 和 CLI 只是两个薄适配层。

## 4. 核心抽象：Tool trait

每个工具是一个纯函数对象，输入 `ToolValue` + 参数，输出 `ToolValue`。

```rust
/// 工具的输入/输出值。统一封装，便于 Pipeline 串联与类型识别。
pub enum ToolValue {
    Text(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    // 流式：处理大文件时避免全量载入内存
    Stream(Box<dyn Read + Send>),
}

/// 工具参数：键值对（UI 表单 / CLI flag 都映射到这里）。
pub type ToolParams = std::collections::BTreeMap<String, ParamValue>;

pub trait Tool: Send + Sync {
    /// 静态元数据：id、分类、名称、参数 schema、Smart Detection 规则等。
    fn descriptor(&self) -> &ToolDescriptor;

    /// 纯执行：无 IO 副作用（文件/剪贴板由 Application 层负责喂入）。
    fn run(&self, input: ToolValue, params: &ToolParams) -> Result<ToolValue, ToolError>;
}
```

### 4.1 ToolDescriptor（元数据，注册表的核心）

```rust
pub struct ToolDescriptor {
    pub id: ToolId,                 // 形如 "encoders.base64"
    pub category: Category,         // Converters / Encoders / ...
    pub name: &'static str,         // "Base64 编解码"
    pub keywords: &'static [&'static str], // 搜索用
    pub params: &'static [ParamSpec],      // UI 表单 & CLI flag 来源
    pub input_kind: IoKind,         // Text / Bytes / Json
    pub output_kind: IoKind,
    pub pipeable: bool,             // 能否参与 Pipeline
    pub detectors: &'static [Detector], // Smart Detection 匹配规则
}
```

> **关键**：UI 表单、CLI 子命令、Pipeline 兼容性校验、Smart Detection 候选，**全部由 descriptor 派生**。新增工具只需实现 `Tool` 并注册，三种形态自动获得支持。

### 4.2 ID 规范
- 格式：`<category>.<tool>`，如 `converters.json_yaml`、`encoders.base64`、`generators.uuid`。
- 分类 enum：`Converters / Encoders / Formatters / Generators / Testers / Text / Graphic / Reference`。

## 5. ToolRegistry（注册表）

Application 层维护全局注册表：启动时收集所有 `Tool` 实例，建立 `id → Tool` 与分类索引。

```rust
pub struct ToolRegistry { /* id -> Arc<dyn Tool>, 分类索引, 关键词索引 */ }

impl ToolRegistry {
    pub fn all(&self) -> impl Iterator<Item = &ToolDescriptor>;
    pub fn by_category(&self, c: Category) -> Vec<&ToolDescriptor>;
    pub fn get(&self, id: &ToolId) -> Option<Arc<dyn Tool>>;
    pub fn search(&self, query: &str) -> Vec<&ToolDescriptor>; // Command Palette
}
```

注册集中在一处（如 `pulsar-app/src/registry.rs` 的 `build_registry()`），便于审计与 feature gate。

## 6. 三种形态如何共享逻辑

```
            ┌─────────────────────────────┐
            │       pulsar-core (Tool)     │
            └──────────────┬──────────────┘
                           │
            ┌──────────────▼──────────────┐
            │   pulsar-app (Registry等)    │
            └───┬───────────┬──────────┬──┘
                │           │          │
        ┌───────▼──┐  ┌─────▼─────┐ ┌──▼────────┐
        │ src-tauri│  │ pulsar-cli│ │ Pipeline  │
        │  (GUI)   │  │  (CLI)    │ │  执行器   │
        └──────────┘  └───────────┘ └───────────┘
```

- **GUI**：Tauri 命令 `run_tool(id, input, params)` → 查注册表 → `tool.run()` → 返回结果。表单字段由 `descriptor.params` 动态渲染。
- **CLI**：`clap` 把子命令映射到 `ToolId`，flag 映射到 `ToolParams`，stdin → `input`，stdout ← 结果。
  ```bash
  pulsar json fmt < in.json
  echo "aGVsbG8=" | pulsar base64 -d
  pulsar uuid --count 5
  ```
- **Pipeline**：见下。

## 7. Pipeline（工具串联，V2）

Pipeline = 一串 `(ToolId, ToolParams)`，前一步输出作为后一步输入。

```rust
pub struct PipelineStep { pub tool: ToolId, pub params: ToolParams }
pub struct Pipeline { pub steps: Vec<PipelineStep> }

// 执行：依次调用，IoKind 兼容性在构建时校验（descriptor.output_kind -> 下一步 input_kind）
fn run_pipeline(reg: &ToolRegistry, p: &Pipeline, input: ToolValue) -> Result<ToolValue, ToolError>;
```

- 构建期用 `descriptor` 校验相邻步骤类型是否兼容，给出 UI 提示。
- Pipeline 可序列化保存（即"工作流"），供 GUI 复用与 CLI 执行：`pulsar run my-flow.yaml < in`。

## 8. Smart Detection（粘贴即识别）✅ 已实现

每个工具在 descriptor 上声明 `detectors: &[Detector]`（声明式、`const` 友好的轻量启发式）。
检测规则与工具逻辑解耦，集中在 `pulsar-core::detect`。

```rust
pub struct Detector { pub rule: Rule, pub confidence: u8 } // 0–100

pub enum Rule {
    JsonParse,                 // 去空白后可解析为 JSON
    Integer,                   // 合法整数
    JwtShape,                  // eyJ 开头的三段 base64url
    Regex(&'static str),       // 自带锚点的正则
    CharsetOnly(&'static str), // 整体由给定字符集组成
}
```

检测入口是注册表方法（注册表已持有全部工具，无需单独类型）：

```rust
// pulsar-app
impl ToolRegistry {
    /// 每个工具取其命中规则的最高置信度，按置信度降序返回候选
    pub fn detect(&self, input: &str) -> Vec<DetectionResult>;
}
pub struct DetectionResult { pub tool_id: String, pub tool_name: String, pub confidence: u8 }
```

- 置信度示例：JWT 95、JSON(格式化) 80、时间戳 75、进制(0x/0o/0b) 70、JSON↔YAML 55、URL 55/45、Base64 40、Hex 35。同一输入多工具命中时按分值排序，UI 展示 Top-N 候选并一键跳转、预填输入。
- IPC：`detect(input) -> Vec<DetectionResult>`；前端 `DetectBar` 去抖 250ms 调用。
- 性能：规则保持 O(短文本) 廉价，`JsonParse` 先按首字节快速排除再解析；对超大输入可后续加长度上限。

## 9. 自动化与流式（接续 tap 基因，V3）

- **ClipboardWatcher**（Infrastructure + Application）：监听剪贴板变化，按用户配置的规则（某检测命中 → 跑某工具/Pipeline → 写回剪贴板或通知）。复用 `tap` 的"监听—响应"思路。
- **流式大文件**：`ToolValue::Stream` 让"哈希校验、Base64、行处理类"工具以流方式处理数百 MB 文件，不爆内存。非流式工具回退到全量。

## 10. 前端架构（React + Zustand + Tailwind）

沿用 `tap` / `wardenly-rs` 的 store-driven 模式，组件保持薄。

| Store | 职责 |
|-------|------|
| `registryStore` | 从后端拉取工具列表/分类/参数 schema |
| `toolStore` | 当前工具的输入/输出/参数、运行状态 |
| `pipelineStore` | Pipeline 步骤编辑与执行（V2） |
| `uiStore` | 视图模式、搜索、收藏、最近使用、Compact 浮窗 |

- **IPC 边界**：所有后端调用走 `lib/ipc.ts`（带 guard，无 Tauri 运行时也能渲染）；事件在 `lib/events.ts` 一处接线。
- **主题**：Tailwind + 语义化 CSS 变量（禁止硬编码颜色），与仓库规范一致。
- **导航**：左侧可搜索分类树 + 顶部 Command Palette（`Cmd/Ctrl+K`），右侧工具面板。

## 11. 持久化

- 历史记录、收藏、保存的工作流：SQLite（与 `wardenly-rs` 一致，`rusqlite` bundled）。
- 配置：YAML（嵌入默认 + 用户覆盖），路径走平台默认目录。
- ID：ULID（时间有序），与仓库统一。

## 12. 可测试性

- `pulsar-core` 每个工具配单测（输入→输出断言），不需 Tauri。
- `pulsar-app` 测注册表、Pipeline 类型校验、SmartDetector 命中。
- 基准：对大文件流式工具加 `criterion` 基准，验证内存/吞吐。

## 13. 开放问题

- formatter 选型：SQL/HTML/CSS/JS 用纯 Rust crate 还是嵌 wasm？（首版可先挑有成熟 Rust 实现的）
- 插件机制：进程内动态库 vs wasm 沙箱 vs 独立子进程？（V4 再定，descriptor 已为外部注册预留结构）
- 图片处理依赖体积与跨平台编译（`image` crate 可行，注意产物体积）。
