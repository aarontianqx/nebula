# tap - 技术选型与项目架构

## 概述

tap（Timed Action Performer）是一个用 Rust 构建的跨平台桌面 GUI 应用，目标是提供“重复操作 → 录制/编辑/重放 → 条件与分支 → 插件扩展”的渐进式自动化能力。

在架构上，我们优先确保：

1. **可靠性与可停止性**：任何时候都能安全停止（全局热键/超时保护）
2. **可演进**：录制、回放、条件、插件都能在不推翻 UI 的前提下迭代
3. **跨平台优先，平台差异可封装**：输入 Hook 与输入模拟差异集中在 infrastructure 层

## 技术栈（选择与理由）

### 语言与构建

- **语言**: Rust（稳定版）
- **构建工具**: Cargo

理由：性能与可靠性（并发/线程安全）、生态成熟、分发便利（单二进制），适合做输入层这种“易出错、需要严谨”的功能。

### GUI 框架

> 决策：**只考虑 Tauri + React**（Win + mac），不再维护 eframe 线路。

- **选择**: **Tauri v2 + React（Vite）**

理由：

- 更容易做出现代、精致、可持续迭代的 UI（布局系统、组件生态、动效与主题）
- 桌面“产品化能力”（托盘、设置页、更新/安装体验等）路线更成熟
- Win + mac 的系统 WebView 可用（Windows 依赖 WebView2 Runtime；macOS 使用系统 WebKit）

> 关键原则：tap 的“自动化引擎”必须与 UI 解耦。这样即使未来 UI 技术栈演进（例如 React 生态升级或替换 UI 框架），也不会影响核心能力。

> 说明：Tauri 的前端栈引入了 React/TS/Vite，但这在“长期维护 + 美观 + 设置/编辑器体验”上性价比更高。

### 数据模型与持久化

- **序列化**: `serde`
- **格式**: MVP 使用 `json`（`serde_json`）

理由：调试友好、生态成熟；后续若更关注可读可写，可迁移到 `ron`/`yaml` 作为“编辑格式”。

### 日志与可观测性

- **日志**: `tracing` + `tracing-subscriber`

理由：结构化日志、过滤方便，后续可以把日志接入 UI 的“事件面板”。

### 输入层（录制/回放的关键依赖，计划选型）

> 本部分是 tap 成败关键，短期先做最小闭环（重复点击/按键），中期引入录制与全局热键。

#### 1) 输入模拟（Output: mouse/keyboard inject）

- **候选**: `enigo`（及其演进版本/社区分支）
- **规划**: **优先评估 `enigo`**，不足时在 Windows 侧补充原生实现（WinAPI SendInput）

理由：enigo 在跨平台“鼠标键盘注入”上较成熟；若遇到游戏/高权限场景不稳定，Windows 可切到更底层的实现。

#### 2) 全局输入监听（Input: record/hook）

- **Windows/Linux**: `rdev`（全局键鼠事件监听/Hook）
- **macOS**: 原生 Core Graphics API（自定义实现）

**macOS 原生实现原因**：

`rdev` 在 macOS 上存在线程安全问题。当处理键盘事件时，`rdev` 会调用 `TSMGetInputSourceProperty` API 来获取键盘字符，但此 API 必须在主线程调用，而 `rdev::listen` 在后台线程的 CFRunLoop 中运行，导致崩溃。

我们的解决方案：
1. 使用 Core Graphics `CGEventTap` API 直接监听全局事件
2. 跳过键盘字符解析，只获取 keycode 并通过静态映射表转换
3. **单例模式**：整个应用共享一个全局事件监听器，避免多个 CGEventTap 冲突
4. **订阅机制**：`mouse_tracker` 和 `input_hook` 都订阅同一个事件流

相关文件：`tap-platform/src/macos_events.rs`

#### 3) 全局热键

- **选择**: `tauri-plugin-global-shortcut`（Tauri v2 插件）
- **实现**: 在 Tauri setup 阶段注册热键，由系统原生机制处理，响应后通过 IPC 通知前端。

### 并发与调度

- **执行模型**: “UI 发命令 → Engine 串行执行（可中断）”
- **实现策略**: MVP 可用 `std::thread + channel`；后续若引入更多异步 I/O，再评估 `tokio`

理由：UI 框架有自己的主线程；执行引擎应独立线程并可被取消，避免 UI 卡顿。

### 插件系统（演进路线）

- **推荐方向**: Wasm 插件
- **候选**:
  - `wasmtime`（通用运行时）
  - `extism`（偏“插件平台”的高层封装）

理由：隔离强、安全性好、跨平台一致；插件 ABI 清晰，适合把“条件判断/识别/自定义动作”外部化。

## 架构设计（分层 + 事件驱动）

tap 采用“分层架构 + 命令/事件”的方式，参考 wardenly-go 的可维护性思路：

```
┌───────────────────────────┐
│      Presentation (GUI)   │  Tauri(React)
│  - Timeline editor        │
│  - Profile manager        │
└──────────────┬────────────┘
               │ Commands (Start/Stop/Record/Replay)
               ▼
┌───────────────────────────┐
│     Application (Engine)  │
│  - Coordinator            │  管理执行任务/录制状态
│  - Player (Replay)        │  时间线回放（可中断）
│  - Recorder (Record)      │  全局事件采集（可暂停）
│  - EventBus               │  事件广播给 UI
└──────────────┬────────────┘
               │ Ports (traits)
               ▼
┌───────────────────────────┐
│   Infrastructure (OS I/O) │
│  - InputInjector          │  注入鼠标键盘
│  - InputHook              │  全局监听（Phase 2）
│  - Storage                │  配置读写
└───────────────────────────┘
               │
               ▼
┌───────────────────────────┐
│         Domain            │
│  - Action / Timeline      │  数据结构与验证规则
│  - Conditions (later)     │
└───────────────────────────┘
```

### 核心设计点

1. **执行引擎串行化**：同一条宏（Timeline）的动作按时间顺序执行，避免并发注入导致不可控行为。
2. **可取消**：所有长任务（回放、录制）都必须支持取消（Stop 信号优先级最高）。
3. **平台差异隔离**：Hook/Inject 的差异不渗透到 UI 和 Domain。
4. **数据可验证**：Timeline 在保存/执行前做校验（非法键名、负延时、过短间隔等）。
5. **全局坐标系统**：所有鼠标坐标基于**全屏物理像素**，而非 WebView 内部相对坐标。

### 全局坐标系统

tap 是一个**全屏自动化工具**，不是"窗口内"的应用。所有鼠标坐标必须基于整个屏幕，而非 WebView 内部坐标。

**关键约束**：

1. **不能依赖 DOM 事件**：WebView 的 `mousemove` / `click` 事件只能捕获窗口内的事件
2. **使用后端全局监听**：通过 `rdev` 在 Rust 层捕获全屏鼠标位置和点击事件
3. **坐标一致性**：录制（rdev）和回放（enigo）使用相同的坐标系

**高 DPI 处理**：

在 Windows 高分屏（如 4K + 150% 缩放）上，坐标系可能不一致。我们通过以下方式统一：

1. **进程启动时声明 Per-Monitor V2 DPI Awareness**：确保获取物理像素坐标
2. **rdev 和 enigo 使用相同坐标系**：都基于 DPI aware 模式工作
3. **`tap-platform/src/dpi.rs`**：提供 DPI 工具函数，必要时进行坐标转换

## 关键流程

### 1) 回放（Replay）流程（目标）

```
UI 点击 Replay
  └─► Engine 校验 Timeline
        └─► Player 启动线程
              ├─► 等待 offset/延时
              ├─► 调用 InputInjector 执行动作
              └─► 检查 Stop 信号（随时中断）
```

### 2) 录制（Record）流程（目标）

```
UI 点击 Record
  └─► Recorder 启动 Hook
        ├─► 收集全局事件 + 时间戳
        ├─► 做降噪/合并（move 采样）
        └─► 生成 Timeline 并推送给 UI
```

## 目录结构

```
tap/                              # 项目根（Cargo workspace + Vite/React root）
│
├── Cargo.toml                    # Cargo workspace 配置
├── package.json                  # yarn/Vite 前端配置
├── vite.config.ts / tsconfig.json
├── index.html                    # Vite 入口 HTML
│
├── src/                          # React 前端源码（UI 层）
│   ├── main.tsx                  # React 入口
│   ├── App.tsx                   # 根组件
│   └── styles.css
│
├── src-tauri/                    # Tauri Rust 后端（桌面壳 + IPC）
│   ├── Cargo.toml
│   ├── src/main.rs               # Rust 入口，暴露 Tauri 命令
│   ├── build.rs                  # 构建脚本（生成 icon.ico 等）
│   ├── tauri.conf.json           # Tauri 应用配置
│   └── icons/                    # 生成的图标
│
├── crates/                       # Rust 核心库（与 UI 解耦）
│   ├── tap-core/                 # 领域模型 + 调度逻辑（无平台依赖）
│   │   └── src/lib.rs
│   └── tap-platform/             # 平台层（输入注入/监听 trait 和实现）
│       └── src/lib.rs
│
├── docs/                         # 文档
│   ├── FUNCTIONAL_GUIDE.md
│   ├── PROJECT_STRUCTURE.md
│   └── UI_DESIGN.md
│
└── README.md
```

### 各 `src` 目录职能说明

| 目录 | 语言/框架 | 职能 |
|------|----------|------|
| `tap/src/` | React + TypeScript | 前端 UI（Vite 构建） |
| `tap/src-tauri/src/` | Rust | Tauri 后端（暴露命令给前端 `invoke` 调用） |
| `tap/crates/tap-core/src/` | Rust | 核心领域模型（`Profile`, `Timeline`, `Action` 等） |
| `tap/crates/tap-platform/src/` | Rust | 平台抽象层（输入注入、全局监听、DPI 处理、macOS 原生事件） |

> **这是 Tauri + Rust workspace 的最佳实践**：前端放 `src/`，Tauri 后端放 `src-tauri/`，可复用的纯 Rust 库放 `crates/`。

## 设计决策（Why）

1. **为什么一开始就选择 Tauri + React？**
   - 你明确目标是 Win + mac 的长期自用，并希望“美观、易用、易迭代”
   - React 生态更适合承载时间线编辑器、脚本/配置编辑器、设置页等复杂 UI
   - 将核心引擎放在 Rust crate 后，前端只做展示与编排，长期维护成本更低

2. **为什么选择 Tauri + React（Win+mac）？**
   - 长期自用更看重“舒服”：现代 UI、设置页、脚本编辑体验、插件管理等更容易维护
   - Web UI 生态成熟，能够更快实现美观一致的交互与信息密度控制
   - 输入 Hook/注入才是硬骨头：将其与 UI 解耦后，UI 选型风险显著降低

3. **为什么强调可取消与紧急停止？**
   - 自动化工具最大的风险是“失控”，必须把 Stop 做成一等公民

4. **为什么插件建议走 Wasm？**
   - ABI 稳定、隔离强，适合承载第三方扩展与用户脚本，降低安全风险

## Phase 3: 条件与识别架构

Phase 3 引入了条件判断和窗口/像素检测能力，架构扩展如下：

### tap-platform 模块详解

`tap-platform` 是平台抽象层，负责隔离操作系统差异：

| 模块 | 职责 | Windows | macOS |
|------|------|---------|-------|
| `injector.rs` | 输入注入（鼠标/键盘） | enigo + 后台线程 | enigo + 后台线程 |
| `input_hook.rs` | 全局事件监听（录制用） | rdev | 原生 CGEventTap（单例） |
| `mouse_tracker.rs` | 全局鼠标位置追踪 | rdev | 原生 CGEventTap（单例） |
| `macos_events.rs` | macOS 原生事件监听（内部模块） | N/A | CGEventTap + 订阅模式 |
| `window.rs` | 窗口 API | Windows API | N/A（待实现） |
| `pixel.rs` | 像素颜色读取 | GDI | CoreGraphics |
| `dpi.rs` | 高 DPI 处理 | SetProcessDpiAwareness | NSScreen scale |

**平台差异处理策略**：

1. **条件编译**：在函数内部使用 `#[cfg(target_os = "...")]` 切换实现
2. **公共接口**：对外暴露统一的 API（如 `start_input_hook()`），内部根据平台选择实现
3. **Trait 抽象**：`InputInjector` trait 允许替换实现（用于测试或未来扩展）

### Phase 3 新增模块

| 模块 | 位置 | 职责 |
|------|------|------|
| `condition.rs` | `tap-core` | 条件类型定义 + 评估器 trait |
| `variables.rs` | `tap-core` | 变量/计数器存储 |
| `window.rs` | `tap-platform` | 窗口 API（获取前台窗口、列表、查找） |
| `pixel.rs` | `tap-platform` | 像素颜色读取 |

### 条件评估流程

```
Timeline 执行
  └─► 遇到 WaitUntil/Conditional 动作
        └─► Player 调用 ConditionEvaluator
              ├─► 窗口检测: PlatformConditionProvider.is_window_focused()
              ├─► 像素检测: PlatformConditionProvider.get_pixel_color()
              └─► 计数器: VariableStore.get_counter()
```

### 目标窗口绑定

Profile 可以绑定到目标窗口（按标题或进程名），执行时：

1. 每个动作前检查目标窗口是否聚焦
2. 如果不匹配且 `pause_when_unfocused=true`，暂停执行
3. 发送 `TargetWindowUnfocused` 事件到前端
4. 窗口重新聚焦后自动恢复

### 新增 Action 类型

| Action | 说明 |
|--------|------|
| `WaitUntil` | 等待条件满足（支持超时） |
| `Conditional` | if/then/else 分支 |
| `SetCounter` | 设置计数器 |
| `IncrCounter` | 计数器 +1 |
| `DecrCounter` | 计数器 -1 |
| `ResetCounter` | 重置计数器 |
| `Exit` | 退出宏执行 |

## Phase 4: 可扩展性架构

Phase 4 引入了 DSL（YAML）支持、参数化变量、子宏调用和表达式引擎。

### 新增模块

| 模块 | 位置 | 职责 |
|------|------|------|
| `dsl.rs` | `tap-core` | YAML DSL 序列化/反序列化 |
| `schema.rs` | `tap-core` | DSL 格式校验 |
| `expression.rs` | `tap-core` | Rhai 表达式引擎（沙箱执行） |
| `submacro.rs` | `tap-core` | 子宏调用管理（防循环、调用栈） |

### DSL 数据流

```
YAML 文件
  └─► parse_yaml() → DslProfile
        └─► validate_profile() → ValidationResult
              └─► import_from_yaml() → Profile
                    └─► Engine 执行
```

### 变量解析流程

```
DslValue (可能含 {{ var }})
  └─► VariableResolver.resolve_dsl_value()
        ├─► 简单引用: 从 VariableStore 获取
        └─► 复杂表达式: ExpressionEngine.evaluate()
              └─► Rhai 沙箱执行
```

### 子宏调用

```
CallMacro { name, args }
  └─► SubMacroContext.push(name) // 检查循环调用
        └─► load_profile(name)
              └─► create_child_variable_store(parent, profile, args)
                    └─► 递归执行子宏
                          └─► SubMacroContext.pop()
```

### 安全性

- **表达式沙箱**：Rhai 引擎禁用文件/网络访问，限制执行深度和操作数
- **循环调用检测**：子宏调用栈跟踪，最大深度 10 层
- **校验**：导入前验证所有必填字段、类型、范围

### 前端 UI

| 功能 | 说明 |
|------|------|
| Code View | YAML 代码编辑器，支持语法高亮提示 |
| Export | 导出当前宏为 YAML 文件 |
| Import | 从 YAML 文件导入宏 |
| Variable Dialog | 执行前填入参数化变量 |
| Validation | 实时校验并显示错误 |

### 文档与模板

- `docs/DSL_REFERENCE.md` - DSL 语法完整参考
- `templates/` - 预置 YAML 模板示例

