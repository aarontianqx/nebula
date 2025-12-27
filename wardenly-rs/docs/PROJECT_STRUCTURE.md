# Wardenly - 架构设计

## 技术栈

| 类别 | 技术 | 说明 |
|------|------|------|
| 后端 | Rust 1.75+ | 高性能、内存安全 |
| 前端 | React 18 + TypeScript | 组件化 UI |
| 桌面框架 | Tauri v2 | 跨平台，轻量级 (~15MB) |
| 样式 | Tailwind CSS v4 | 实用优先 |
| 浏览器自动化 | chromiumoxide | CDP 协议，纯 Rust |
| 异步运行时 | tokio | 高性能异步 I/O |
| 数据库 | rusqlite / MongoDB | 本地/远程存储 |
| 键盘监听 | rdev | 跨平台系统输入 |
| 日志 | tracing | 结构化日志 |

## 架构原则

### 1. 分层架构 (DDD + Onion)

依赖方向从外向内，内层不依赖外层：

```
┌─────────────────────────────────────────────────────────────┐
│                     Adapter 层                               │
│  Tauri IPC、前端通信                                         │
├─────────────────────────────────────────────────────────────┤
│                   Application 层                             │
│  用例编排、Coordinator、EventBus、InputProcessor             │
├─────────────────────────────────────────────────────────────┤
│                  Infrastructure 层                           │
│  数据库、浏览器驱动、键盘监听、配置加载                        │
├─────────────────────────────────────────────────────────────┤
│                     Domain 层                                │
│  实体、值对象、Repository 接口、领域事件                      │
└─────────────────────────────────────────────────────────────┘
```

| 层 | 职责 | 依赖 |
|---|------|------|
| **Domain** | 业务实体、值对象、领域事件、Repository 接口 | 无 |
| **Application** | 用例编排、命令处理、事件总线 | Domain |
| **Infrastructure** | 数据库、浏览器、键盘、配置 | Domain |
| **Adapter** | Tauri IPC、前端通信 | Application |

### 2. 事件驱动

- **命令 (Command)**: 用户意图 (`CreateSession`, `Click`)
- **事件 (Event)**: 状态变化 (`SessionCreated`, `StateChanged`)
- **事件总线**: `tokio::sync::broadcast` 发布-订阅

```
Frontend invoke() → Tauri Command → Coordinator → Session
                                                     │
Frontend listen() ← Tauri emit() ← EventBus ←───── Event
```

### 3. Actor 模式

每个 Session 作为独立 Actor：
- 通过 `mpsc` channel 接收命令
- 串行处理保证线程安全
- 自主管理生命周期

### 4. 平台抽象

平台特定代码通过 trait 隔离：

```
Application 层: InputEventProcessor (平台无关)
        ▲
Infrastructure 层: KeyboardListener trait
        ▲
   ┌────┼────┐
macOS  Windows  Linux
```

## 核心组件

### Session 状态机

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

### Coordinator

协调器管理所有 Session：
- 路由命令到对应 Session
- 处理批量操作 (ClickAll, StartAllScripts)
- 监听 SessionStopped 清理资源

### InputEventProcessor

处理键盘透传：

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

### ScriptRunner

脚本执行器：
- 截取画面并匹配场景
- 执行动作序列
- 支持循环和条件控制

## 目录结构

```
wardenly-rs/
├── src/                            # 前端 (React + TypeScript)
│   ├── App.tsx                     # 应用入口组件
│   ├── main.tsx                    # React 入口
│   ├── components/
│   │   ├── layout/
│   │   │   └── MainWindow.tsx      # 主窗口布局
│   │   ├── session/
│   │   │   ├── SessionList.tsx     # 会话列表
│   │   │   └── ScriptControls.tsx  # 脚本控制面板
│   │   ├── canvas/
│   │   │   └── CanvasWindow.tsx    # 画布窗口
│   │   ├── dialogs/
│   │   │   └── ManagementDialog.tsx # 管理对话框
│   │   └── forms/
│   │       ├── AccountForm.tsx     # 账户表单
│   │       └── GroupForm.tsx       # 分组表单
│   ├── hooks/
│   │   └── useTauriEvents.ts       # Tauri 事件监听 Hook
│   ├── stores/
│   │   ├── accountStore.ts         # 账户状态 (Zustand)
│   │   └── sessionStore.ts         # 会话状态 (Zustand)
│   ├── types/
│   │   └── index.ts                # TypeScript 类型定义
│   └── styles/
│       └── globals.css             # 全局样式
│
├── src-tauri/                      # 后端 (Rust + Tauri)
│   ├── src/
│   │   ├── main.rs                 # 应用入口
│   │   ├── lib.rs                  # 库入口，依赖注入
│   │   │
│   │   ├── domain/                 # 领域层 (最内层)
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
│   │   ├── application/            # 应用层
│   │   │   ├── service/
│   │   │   │   ├── session_actor.rs    # Session Actor
│   │   │   │   ├── account_service.rs  # 账户服务
│   │   │   │   ├── group_service.rs    # 分组服务
│   │   │   │   └── script_runner.rs    # 脚本执行器
│   │   │   ├── input/
│   │   │   │   ├── processor.rs    # InputEventProcessor
│   │   │   │   └── gesture.rs      # GestureRecognizer
│   │   │   ├── command.rs          # 命令定义
│   │   │   ├── coordinator.rs      # 多会话协调器
│   │   │   └── eventbus.rs         # 事件总线
│   │   │
│   │   ├── infrastructure/         # 基础设施层
│   │   │   ├── persistence/
│   │   │   │   ├── sqlite/         # SQLite 实现
│   │   │   │   └── mongodb/        # MongoDB 实现 (可选)
│   │   │   ├── browser/
│   │   │   │   ├── driver.rs       # BrowserDriver trait
│   │   │   │   └── chromium.rs     # chromiumoxide 实现
│   │   │   ├── input/
│   │   │   │   ├── keyboard.rs     # KeyboardListener trait
│   │   │   │   ├── macos.rs        # macOS 实现
│   │   │   │   ├── windows.rs      # Windows 实现
│   │   │   │   └── linux.rs        # Linux 实现
│   │   │   ├── config/
│   │   │   │   ├── loader.rs       # YAML 加载器
│   │   │   │   ├── paths.rs        # 平台路径
│   │   │   │   ├── app_config.rs   # 应用配置
│   │   │   │   ├── gesture_config.rs # 手势配置
│   │   │   │   └── resources.rs    # 资源加载
│   │   │   └── logging/            # 日志模块
│   │   │
│   │   └── adapter/                # 适配器层
│   │       └── tauri/
│   │           ├── commands.rs     # Tauri 命令
│   │           ├── events.rs       # 事件推送
│   │           ├── state.rs        # 应用状态
│   │           └── error.rs        # API 错误处理
│   │
│   ├── resources/                  # 嵌入式资源
│   │   ├── configs/
│   │   │   ├── app.yaml            # 应用配置
│   │   │   └── gesture.yaml        # 手势配置
│   │   ├── scenes/                 # 场景定义 (*.yaml)
│   │   └── scripts/                # 脚本定义 (*.yaml)
│   │
│   ├── icons/                      # 应用图标
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── docs/                           # 文档
│   ├── FUNCTIONAL_GUIDE.md         # 功能说明
│   ├── PROJECT_STRUCTURE.md        # 架构设计
│   ├── UI_DESIGN.md                # UI 设计
│   └── roadmap/                    # 开发路线图
│
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── tsconfig.json
└── README.md
```

## 设计决策

| 决策 | 理由 |
|------|------|
| **Actor 模式** | Session 状态复杂，串行处理避免竞争 |
| **broadcast channel** | 解耦事件发布/订阅，支持多订阅者 |
| **Tauri v2** | Web UI 灵活，体积小 |
| **双存储支持** | SQLite 本地优先，MongoDB 多设备同步 |
| **chromiumoxide** | CDP 功能丰富，纯 Rust |
| **rdev** | 跨平台键盘监听，API 统一 |
| **仅 A-Z 透传** | 避免与系统快捷键冲突 |
