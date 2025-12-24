# Phase 5: 架构优化与跨平台成熟度

## 状态：✅ 已完成

子模块分离重构已完成，`tap-platform` 现在采用清晰的子目录结构。

## 背景

在 Phase 4 完成后，我们解决了 macOS 上 `rdev` 库的线程安全问题，实现了原生 Core Graphics 事件监听。随后进行了架构优化，将平台代码分离为独立子模块。

## 已完成的优化

### 子模块分离

`tap-platform` 现在采用以下目录结构：

```
tap-platform/src/
├── lib.rs                 # 根模块：re-export 所有公共 API
├── error.rs               # PlatformError 定义
├── injector.rs            # 输入注入（全平台共用 enigo）
├── events/                # 事件监听子模块
│   ├── mod.rs             # 公共类型 + 入口函数
│   └── macos.rs           # macOS 原生实现（CGEventTap 单例）
├── input_hook/            # 全局输入钩子子模块
│   ├── mod.rs             # RawInputEvent, InputHookHandle 等公共类型
│   ├── rdev_impl.rs       # Windows/Linux 实现
│   └── macos.rs           # macOS 实现
├── mouse_tracker/         # 鼠标追踪子模块
│   ├── mod.rs             # MousePosition, MouseTrackerHandle 等公共类型
│   ├── rdev_impl.rs       # Windows/Linux 实现
│   └── macos.rs           # macOS 实现
├── window/                # 窗口 API 子模块
│   ├── mod.rs             # WindowInfo, WindowRect 等公共类型
│   ├── windows.rs         # Windows 实现
│   └── macos.rs           # macOS 实现（待完善）
├── pixel/                 # 像素检测子模块
│   ├── mod.rs             # Color 类型 + 公共接口
│   ├── windows.rs         # Windows GDI 实现
│   └── macos.rs           # macOS 实现（待完善）
└── dpi/                   # DPI 处理子模块
    ├── mod.rs             # ScaledCoords 类型
    ├── windows.rs         # Windows DPI API
    └── macos.rs           # macOS 实现
```

### 架构优点

1. **清晰的分层**：`tap-core`（业务逻辑）与 `tap-platform`（平台 I/O）完全分离
2. **平台无关的核心**：`tap-core` 不依赖任何平台特定代码
3. **Trait 抽象**：`InputInjector`、`ConditionEvaluator` 定义了清晰的接口边界
4. **单例事件监听**：macOS 使用 `GlobalEventListener` 单例，避免多 CGEventTap 冲突
5. **子模块分离**：每个功能领域有独立目录，平台实现分离为独立文件
6. **易于扩展**：添加新平台只需在对应子目录下添加实现文件

### 未保留的模块

- **`injector.rs`**：全平台共用 enigo + 后台线程模式，无需分离，保持为单文件

## 待完成

1. **macOS 窗口 API**：`window/macos.rs` 目前返回空值，需使用 Accessibility API 实现
2. **macOS 像素读取**：`pixel/macos.rs` 目前返回空值，需使用 CGDisplayCreateImageForRect 实现

## 未来考虑

### Linux 支持

如果未来需要支持 Linux：

1. `rdev` 在 Linux 上工作良好（X11/Wayland）
2. 在 `input_hook/`、`mouse_tracker/` 中添加 `linux.rs`（或复用 `rdev_impl.rs`）
3. 在 `window/`、`pixel/`、`dpi/` 中添加 `linux.rs`

### 插件系统

Phase 4 提到的 Wasm 插件系统：

1. 使用 `wasmtime` 或 `extism` 运行 Wasm 插件
2. 定义插件 ABI（条件判断、自定义动作）
3. 沙箱隔离确保安全

## 决策记录

| 日期 | 决策 | 理由 |
|------|------|------|
| 2024-12 | 实现 macOS 原生事件监听 | rdev 线程安全问题导致崩溃 |
| 2024-12 | 采用单例模式 | 避免多个 CGEventTap 冲突 |
| 2024-12 | 子模块分离重构 | 提高代码组织清晰度，便于未来扩展 |

## 结论

**当前架构评级：✅ 优秀**

子模块分离重构已完成，架构清晰、易于维护和扩展。下一步可以：
1. 补充 macOS 窗口和像素 API 实现
2. 考虑添加 Linux 支持
3. 评估插件系统需求
