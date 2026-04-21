# Wardenly - 项目架构

## 概述

Wardenly 是一个基于 Go 和 Fyne 构建的桌面应用程序，用于管理 WLY 网页游戏的浏览器自动化任务。系统采用清晰的分层架构和事件驱动设计，支持多账户并发运行和自动化脚本执行。

## 技术栈

- **语言**: Go 1.23+
- **UI 框架**: Fyne v2.5.2 (跨平台 GUI)
- **浏览器自动化**: ChromeDP (headless Chrome 驱动)
- **数据库**: MongoDB (账户持久化)
- **日志**: slog + lumberjack (滚动日志)

## 架构设计原则

1. **分层架构**: 清晰的职责边界，从 presentation 到 infrastructure
2. **Actor 模式**: Session 作为独立的 Actor，通过命令队列串行处理命令
3. **事件驱动**: 使用 EventBus 解耦组件间通信
4. **命令-事件分离**: Command 表示用户意图，Event 表示状态变化

## 目录结构

```
wardenly-go/
├── cmd/wardenly/               # 应用程序入口
│   └── main.go                 # 初始化和依赖注入
│
├── core/                       # 核心抽象层
│   ├── command/                # 命令定义
│   │   ├── command.go          # Command/SessionCommand 接口
│   │   ├── browser_ops.go      # 浏览器操作命令 (Click, Drag, CaptureScreen 等)
│   │   ├── script_ctrl.go      # 脚本控制命令 (StartScript, StopScript 等)
│   │   └── session_lifecycle.go # 会话生命周期命令 (StartSession, StopSession 等)
│   │
│   ├── event/                  # 事件定义
│   │   ├── event.go            # Event/SessionEvent 接口，会话事件
│   │   ├── browser_events.go   # 浏览器事件 (ScreenCaptured, LoginSucceeded 等)
│   │   └── script_events.go    # 脚本事件 (ScriptStarted, ScriptStopped 等)
│   │
│   ├── eventbus/               # 事件总线
│   │   ├── eventbus.go         # EventBus 接口
│   │   └── impl.go             # 异步事件总线实现
│   │
│   └── state/                  # 状态机
│       └── state.go            # SessionState 定义和转换规则
│
├── domain/                     # 领域模型层
│   ├── account/                # 账户领域
│   │   ├── account.go          # Account 实体 (ID, RoleName, Cookies 等)
│   │   ├── repository.go       # Repository 接口
│   │   └── service.go          # 领域服务
│   │
│   ├── group/                  # 分组领域
│   │   ├── group.go            # Group 实体 (ID, Name, AccountIDs)
│   │   ├── repository.go       # Repository 接口
│   │   └── service.go          # 领域服务（含账户解析）
│   │
│   ├── scene/                  # 场景识别领域
│   │   ├── scene.go            # Scene 实体，颜色点匹配
│   │   ├── registry.go         # 场景注册表
│   │   └── loader.go           # YAML 加载器
│   │
│   └── script/                 # 自动化脚本领域
│       ├── script.go           # Script, Step, Action 定义
│       ├── registry.go         # 脚本注册表
│       └── loader.go           # YAML 加载器
│
├── application/                # 应用层
│   ├── coordinator.go          # 会话协调器，管理多会话和跨会话操作
│   └── session/                # 会话 Actor
│       ├── session.go          # Session Actor 实现
│       ├── browser_ctrl.go     # 浏览器控制器
│       ├── screen_capture.go   # 屏幕截图
│       └── script_runner.go    # 脚本执行引擎
│
├── presentation/               # 表示层 (UI)
│   ├── main_window.go          # 主窗口，工具栏和侧边栏布局
│   ├── session_list.go         # 会话列表侧边栏
│   ├── session_tab.go          # 单个会话的控制面板
│   ├── management_dialog.go    # 账户/分组管理对话框
│   ├── account_form.go         # 账户编辑表单
│   ├── group_form.go           # 分组编辑表单
│   ├── canvas_window.go        # 浏览器画布窗口
│   ├── canvas_manager.go       # 画布生命周期管理
│   ├── screencast_manager.go   # 帧流管理
│   └── bridge.go               # UI-应用层事件桥接
│
├── infrastructure/             # 基础设施层
│   ├── browser/                # 浏览器驱动
│   │   ├── driver.go           # Driver 接口定义
│   │   └── chromedp_driver.go  # ChromeDP 实现
│   │
│   ├── logging/                # 日志基础设施
│   │   ├── config.go           # 配置和全局 logger 访问
│   │   ├── setup_dev.go        # 开发环境：控制台输出
│   │   └── setup_prod.go       # 生产环境：滚动文件
│   │
│   ├── ocr/                    # OCR 服务
│   │   └── client.go           # HTTP OCR 客户端
│   │
│   └── repository/             # 数据持久化
│       ├── mongodb.go          # MongoDB 连接管理
│       ├── account_repo.go     # 账户仓库实现
│       └── group_repo.go       # 分组仓库实现
│
├── resources/                  # 嵌入式资源
│   ├── resources.go            # embed.FS 声明
│   ├── icons/                  # 应用图标
│   ├── scenes/                 # 场景定义 YAML
│   ├── scripts/                # 脚本定义 YAML
│   └── snapshots/              # 场景截图参考
│
├── tools/                      # 开发工具
│   ├── scene-analyzer/         # 场景分析工具
│   ├── scene-generator/        # 场景生成工具
│   └── migrate-groups/         # 分组数据迁移工具
│
├── build.sh                    # Linux/macOS 构建脚本
├── build.ps1                   # Windows PowerShell 构建脚本
└── go.mod                      # Go 模块定义
```

## 核心组件详解

### 1. Session Actor (`application/session/session.go`)

Session 是整个系统的核心，采用 Actor 模式实现：

```
                  ┌─────────────────────────┐
                  │        Session          │
                  │  (Actor, 串行处理命令)    │
                  ├─────────────────────────┤
  Command ───────►│  cmdChan (命令队列)      │
                  ├─────────────────────────┤
                  │  State Machine          │
                  │  ┌─────────────────┐    │
                  │  │ Idle → Starting │    │
                  │  │   ↓             │    │
                  │  │ LoggingIn       │    │
                  │  │   ↓             │    │
                  │  │ Ready ⇄ Script  │    │
                  │  │   ↓    Running  │    │
                  │  │ Stopped         │    │
                  │  └─────────────────┘    │
                  ├─────────────────────────┤
                  │  Components:            │
                  │  - BrowserController    │
                  │  - ScreenCapture        │
                  │  - ScriptRunner         │
  Event ◄─────────│  EventBus.Publish()     │
                  └─────────────────────────┘
```

**状态转换规则**:
- `Idle` → `Starting`: 会话开始
- `Starting` → `LoggingIn`: 浏览器启动成功
- `LoggingIn` → `Ready`: 登录成功
- `Ready` ⇄ `ScriptRunning`: 脚本启动/停止
- 任意状态 → `Stopped`: 会话终止

### 2. Coordinator (`application/coordinator.go`)

协调器管理多个 Session 实例：

```
┌───────────────────────────────────────────┐
│               Coordinator                  │
├───────────────────────────────────────────┤
│  sessions: map[string]*Session            │
│                                           │
│  Dispatch(cmd) ─────┬─► Session 命令路由   │
│                     └─► 多会话命令处理     │
│     - ClickAll: 向所有活跃会话发送点击     │
│     - StartAllScripts: 批量启动脚本       │
│     - SyncScriptSelection: 同步脚本选择   │
│                                           │
│  EventBus 订阅 ←── 监听 SessionStopped    │
└───────────────────────────────────────────┘
```

### 3. 事件驱动架构

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Presentation │     │   EventBus   │     │  Application │
│    (UI)       │     │   (异步)      │     │  (Session)   │
├──────────────┤     ├──────────────┤     ├──────────────┤
│              │     │              │     │              │
│  Command ────┼────►│              │     │              │
│              │     │              │     │  ◄─── 处理   │
│              │     │              │     │              │
│  ◄─── 更新   │     │   ◄───────── │     │   Event ────►│
│              │     │   广播事件   │     │              │
└──────────────┘     └──────────────┘     └──────────────┘

主要事件流:
1. UI 发送 Command → Coordinator → Session
2. Session 处理后发布 Event → EventBus
3. EventBus 广播 → UIEventBridge 接收 → UI 更新
```

### 4. UI 组件层次

```
MainWindow
├── Toolbar (账户选择、分组运行、选项)
├── SessionList (左侧边栏，会话列表)
├── DetailPanel (右侧，当前会话详情)
│   └── SessionTab (会话控制面板)
│       ├── 浏览器控制 (Stop, Refresh, Save Cookies)
│       ├── 脚本控制 (Start/Stop Script, 脚本选择)
│       └── 画布控制 (坐标显示，点击操作)
│
├── CanvasManager (画布生命周期管理)
│   └── CanvasWindow (独立窗口显示浏览器画面)
│
└── ScreencastManager (帧流管理)
    └── 控制 screencast 的启动/停止/切换
```

### 5. 浏览器驱动 (`infrastructure/browser/`)

```
┌─────────────────────────────────────────┐
│          Driver Interface               │
├─────────────────────────────────────────┤
│  Start(ctx) / Stop()                    │
│  Navigate(url) / Reload()               │
│  Click(x, y) / Drag(from, to)           │
│  CaptureScreen() → image.Image          │
│  StartScreencast() → chan image.Image   │
│  LoginWithCookies() / LoginWithPassword()│
└─────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────┐
│        ChromeDPDriver                   │
├─────────────────────────────────────────┤
│  Headless Chrome 实现                    │
│  窗口大小: 1080x840                      │
│  视口大小: 1080x720                      │
│  Screencast: 质量 80, 最大 5 FPS         │
└─────────────────────────────────────────┘
```

## 数据流

### 登录流程

```
1. 用户点击 "Run Account"
   │
   ▼
2. MainWindow.runAccount()
   ├── 创建 SessionTab
   ├── 注册到 CanvasManager
   ├── 添加到 SessionList
   └── 发送 StartSession 命令
   │
   ▼
3. Coordinator.handleStartSession()
   └── 创建 Session, 调用 StartBrowser()
   │
   ▼
4. Session.StartBrowser()
   ├── 状态: Idle → Starting → LoggingIn
   ├── 发布 DriverStarted 事件
   └── 启动 performLogin() goroutine
   │
   ▼
5. ScreencastManager 收到 DriverStarted
   └── 延迟 1 秒后发送 StartScreencast 命令
   │
   ▼
6. Session.performLogin()
   ├── 使用 Cookies 或 用户名密码 登录
   ├── 等待游戏加载 (场景识别)
   ├── 保存新 Cookies
   └── 状态: LoggingIn → Ready
   │
   ▼
7. 发布 LoginSucceeded 事件
   └── UI 启用控制按钮
```

### 脚本执行流程

```
1. 用户点击 "Start Script"
   │
   ▼
2. 发送 StartScript 命令
   │
   ▼
3. Session 处理
   ├── 状态: Ready → ScriptRunning
   └── ScriptRunner.Start()
   │
   ▼
4. ScriptRunner 执行循环
   ├── 截图 → 场景匹配 → 执行动作
   ├── 支持循环、条件退出、OCR 检测
   └── 继续直到手动停止或条件触发
   │
   ▼
5. 脚本停止
   ├── 状态: ScriptRunning → Ready
   └── 发布 ScriptStopped 事件
```

## 日志系统

日志通过 build tag 区分环境：

| 环境 | Build Tag | 输出目标 | 日志级别 |
|------|-----------|----------|----------|
| 开发 | (无) | 控制台 | Debug |
| 生产 | `-tags prod` | 滚动文件 | Debug |

生产环境日志位置: `~/.config/wardenly/logs/` (Windows: `%APPDATA%\wardenly\logs\`)

## 构建

```bash
# 开发构建
./build.sh

# 生产构建 (启用日志文件，隐藏控制台)
./build.sh -prod

# Windows
.\build.ps1
.\build.ps1 -prod
```

## 设计决策

1. **为什么使用 Actor 模式？**
   - Session 包含大量状态（浏览器、脚本、截图）
   - 串行命令处理避免数据竞争
   - 清晰的生命周期管理

2. **为什么使用 EventBus 而非直接回调？**
   - 解耦 UI 和业务逻辑
   - 支持多订阅者
   - 异步非阻塞

3. **为什么 CanvasManager 使用命令队列？**
   - Fyne UI 更新必须在主线程
   - 多会话切换时避免竞态条件
   - 统一的帧更新节流

4. **为什么 Screencast 延迟 1 秒启动？**
   - 避免浏览器初始化期间的空白帧
   - 给登录流程预留时间
   - 确保 driver 完全就绪

