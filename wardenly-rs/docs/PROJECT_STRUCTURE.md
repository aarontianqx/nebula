# Wardenly - 项目架构

## 概述

Wardenly 是一个基于 Rust 和 Tauri 构建的跨平台桌面应用程序，用于管理 WLY 网页游戏的浏览器自动化任务。系统采用清晰的分层架构和事件驱动设计，支持多账户并发运行和自动化脚本执行。

## 技术栈

| 类别 | 技术选型 | 说明 |
|------|----------|------|
| **语言** | Rust 1.75+ / TypeScript | 后端 / 前端 |
| **桌面框架** | Tauri v2 | 跨平台，轻量级 |
| **前端框架** | React 18 + Tailwind CSS | 组件化 UI |
| **浏览器自动化** | chromiumoxide | CDP 协议，纯 Rust |
| **异步运行时** | tokio | 高性能异步 I/O |
| **数据库** | SeaORM | 支持 SQLite / MongoDB |
| **系统输入** | rdev | 跨平台键盘监听 |
| **日志** | tracing | 结构化日志 |

## 架构设计原则

### 1. 领域驱动设计 (DDD) + 洋葱架构

依赖方向从外向内，内层不依赖外层：

```
┌─────────────────────────────────────────────────────────────┐
│                     Adapter 层                               │
│  (Tauri IPC, 前端通信)                                       │
├─────────────────────────────────────────────────────────────┤
│                   Application 层                             │
│  (用例编排, Coordinator, EventBus, InputProcessor)           │
├─────────────────────────────────────────────────────────────┤
│                  Infrastructure 层                           │
│  (数据库, 浏览器驱动, 键盘监听, 配置加载)                      │
├─────────────────────────────────────────────────────────────┤
│                     Domain 层                                │
│  (实体, 值对象, Repository接口, 领域事件)                     │
└─────────────────────────────────────────────────────────────┘
```

- **聚合根**: Account、Group
- **值对象**: Scene、Script (不可变)
- **Repository**: 接口定义在 Domain 层，实现在 Infrastructure 层

### 2. 事件驱动架构

- **命令 (Command)**: 表示用户意图，如 `CreateSession`, `Click`
- **事件 (Event)**: 表示已发生的状态变化，如 `SessionCreated`, `StateChanged`
- **事件总线**: 使用 `tokio::sync::broadcast` 实现发布-订阅

### 3. Actor 模式

每个 Session 作为独立 Actor：
- 通过 `mpsc` channel 接收命令
- 串行处理保证线程安全
- 自主管理生命周期和资源释放

### 4. 平台特定代码处理

平台特定代码封装在 Infrastructure 层，通过 trait 暴露统一接口：

```
Application 层: InputEventProcessor (平台无关)
        ▲
        │ KeyEvent
Infrastructure 层: KeyboardListener trait
        ▲
   ┌────┼────┐
MacOS  Windows  Linux
```

## 目录结构

```
wardenly-rs/
├── src-tauri/                      # Rust 后端
│   ├── src/
│   │   ├── main.rs                 # Tauri 入口，依赖注入
│   │   ├── lib.rs
│   │   │
│   │   ├── domain/                 # 🎯 领域层 (最内层)
│   │   │   ├── model/
│   │   │   │   ├── account.rs      # Account 聚合根
│   │   │   │   ├── group.rs        # Group 聚合根
│   │   │   │   ├── session.rs      # Session 实体 + 状态机
│   │   │   │   ├── scene.rs        # Scene 值对象
│   │   │   │   └── script.rs       # Script 值对象
│   │   │   ├── repository.rs       # Repository trait
│   │   │   ├── event.rs            # 领域事件
│   │   │   └── error.rs            # 领域错误
│   │   │
│   │   ├── application/            # 📦 应用层
│   │   │   ├── service/
│   │   │   │   ├── session_service.rs   # SessionActor
│   │   │   │   ├── account_service.rs   # Account CRUD
│   │   │   │   └── script_service.rs    # ScriptRunner
│   │   │   ├── input/
│   │   │   │   ├── processor.rs    # InputEventProcessor
│   │   │   │   └── gesture.rs      # GestureRecognizer
│   │   │   ├── command.rs          # 命令定义
│   │   │   ├── coordinator.rs      # 多会话协调器
│   │   │   └── eventbus.rs         # 事件总线
│   │   │
│   │   ├── infrastructure/         # 🔌 基础设施层
│   │   │   ├── persistence/
│   │   │   │   ├── sqlite/         # SQLite 实现
│   │   │   │   └── mongodb/        # MongoDB 实现
│   │   │   ├── browser/
│   │   │   │   ├── driver.rs       # BrowserDriver trait
│   │   │   │   └── chromium.rs     # chromiumoxide 实现
│   │   │   ├── input/
│   │   │   │   ├── keyboard.rs     # KeyboardListener trait
│   │   │   │   ├── macos.rs
│   │   │   │   ├── windows.rs
│   │   │   │   └── linux.rs
│   │   │   ├── config/
│   │   │   │   ├── loader.rs       # YAML 加载
│   │   │   │   ├── paths.rs        # 平台路径
│   │   │   │   ├── app_config.rs
│   │   │   │   ├── gesture_config.rs
│   │   │   │   └── resources.rs    # 场景/脚本加载
│   │   │   ├── ocr/
│   │   │   └── logging/
│   │   │
│   │   └── adapter/                # 🔗 适配器层
│   │       └── tauri/
│   │           ├── commands.rs     # #[tauri::command]
│   │           ├── events.rs       # 事件推送
│   │           ├── state.rs        # Tauri State
│   │           └── error.rs        # API 错误处理
│   │
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── src/                            # 前端 (React + TypeScript)
│   ├── App.tsx
│   ├── components/
│   │   ├── layout/
│   │   ├── session/
│   │   ├── canvas/
│   │   └── management/
│   ├── hooks/
│   ├── stores/
│   ├── types/
│   └── styles/
│
├── resources/                      # 嵌入式资源
│   ├── configs/
│   │   ├── app.yaml                # 应用配置
│   │   └── gesture.yaml            # 手势配置
│   ├── scenes/
│   ├── scripts/
│   ├── snapshots/
│   └── icons/
│
└── docs/
    ├── FUNCTIONAL_GUIDE.md
    ├── PROJECT_STRUCTURE.md
    ├── UI_DESIGN.md
    └── roadmap/                    # 开发路线图
```

### 层次职责

| 层 | 职责 | 依赖 |
|---|------|------|
| **Domain** | 业务实体、值对象、领域事件、Repository 接口 | 无 |
| **Application** | 用例编排、命令处理、事件总线、输入处理 | Domain |
| **Infrastructure** | 数据库、浏览器驱动、键盘监听、配置 | Domain |
| **Adapter** | Tauri IPC、前端通信 | Application |

## 核心组件

### 1. Session 状态机

```
Idle → Starting → LoggingIn → Ready ⇄ ScriptRunning
                     │           │
                     └───────────┴──────→ Stopped
```

| 状态 | 说明 | 允许操作 |
|------|------|----------|
| Idle | 初始 | - |
| Starting | 浏览器启动中 | - |
| LoggingIn | 登录中 | 点击/拖拽 |
| Ready | 待机 | 所有操作 |
| ScriptRunning | 脚本运行中 | 停止脚本 |
| Stopped | 已结束 | - |

### 2. Coordinator

协调器管理所有 Session 实例：
- 路由命令到对应 Session
- 处理跨会话批量操作 (ClickAll, StartAllScripts)
- 监听 SessionStopped 清理资源

### 3. InputEventProcessor (Keyboard Passthrough)

处理系统键盘事件，转换为画布点击：

```
系统键盘 → KeyboardListener → GestureRecognizer → InputEventProcessor → Coordinator
                                    │
                            识别 Tap/LongPress
```

**GestureRecognizer 状态机**:
- **Tap**: 按下后 <300ms 释放
- **LongPressStart**: 按下超过 300ms
- **LongPressRepeat**: 按住期间每 100ms 触发
- **LongPressEnd**: 释放

### 4. 事件驱动数据流

```
Frontend invoke() → Tauri Command → Coordinator → Session
                                                     │
Frontend listen() ← Tauri emit() ← EventBus ←──── Event
```

## 配置系统

### 配置文件

**`resources/configs/app.yaml`**:
```yaml
storage:
  type: sqlite          # sqlite 或 mongodb
  sqlite:
    path: ""            # 留空使用平台默认路径
  mongodb:
    uri: "mongodb://localhost:27017"
    database: "wardenly"

browser:
  chrome_path: ""       # 留空自动查找
  window_width: 1080
  window_height: 840
```

**`resources/configs/gesture.yaml`**:
```yaml
keyboard_passthrough:
  long_press_threshold_ms: 300
  repeat_interval_ms: 100
  debounce_window_ms: 50
```

### 平台默认路径

| 平台 | 配置目录 | 数据库 |
|------|----------|--------|
| macOS | `~/Library/Application Support/wardenly/` | `data.db` |
| Linux | `~/.config/wardenly/` | `data.db` |
| Windows | `%APPDATA%\wardenly\` | `data.db` |

## 依赖库

```toml
[dependencies]
tauri = { version = "2" }
tokio = { version = "1", features = ["full"] }
chromiumoxide = { version = "0.7", features = ["tokio-runtime"] }
sea-orm = { version = "1.0", features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
image = "0.25"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
rdev = "0.5"
dirs = "5"
thiserror = "2"
anyhow = "1"
async-trait = "0.1"

[dependencies.mongodb]
version = "3.1"
features = ["tokio-runtime"]
optional = true

[features]
default = ["sqlite"]
sqlite = []
mongodb = ["dep:mongodb"]
```

## 设计决策

| 决策 | 理由 |
|------|------|
| **Actor 模式** | Session 状态复杂，串行处理避免竞争 |
| **broadcast channel** | 解耦事件发布者/订阅者，支持多订阅 |
| **Tauri v2** | Web UI 灵活，体积小 (~15MB) |
| **双存储支持** | SQLite 本地优先，MongoDB 多设备同步 |
| **chromiumoxide** | CDP 功能丰富，纯 Rust，性能好 |
| **rdev** | 跨平台键盘监听，API 统一 |

## 开发路线图

详见 [docs/roadmap/](./roadmap/ROADMAP.md)
