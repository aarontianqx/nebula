# Phase 5: 架构优化与跨平台成熟度

## 状态：提案（低优先级）

当前架构已经满足 Windows + macOS 双平台需求，本提案记录未来可考虑的优化方向。

## 背景

在 Phase 4 完成后，我们解决了 macOS 上 `rdev` 库的线程安全问题，实现了原生 Core Graphics 事件监听。当前架构评估：

### ✅ 已良好实现

1. **清晰的分层**：`tap-core`（业务逻辑）与 `tap-platform`（平台 I/O）完全分离
2. **平台无关的核心**：`tap-core` 不依赖任何平台特定代码
3. **Trait 抽象**：`InputInjector`、`ConditionEvaluator` 定义了清晰的接口边界
4. **单例事件监听**：macOS 使用 `GlobalEventListener` 单例，避免多 CGEventTap 冲突

### ⚠️ 可改进但非阻塞

1. **平台代码组织**：条件编译分散在多个文件中
2. **部分功能缺失**：macOS 窗口 API 未完全实现

## 可选优化方案

### 方案 A：子模块分离（推荐，如需进行）

将 `tap-platform` 内部按平台分离为子模块：

```
tap-platform/src/
├── lib.rs              # 公共接口 + 类型定义
├── injector/
│   ├── mod.rs          # InputInjector trait
│   ├── enigo.rs        # 跨平台 enigo 实现
│   └── windows.rs      # Windows 特定优化（可选）
├── events/
│   ├── mod.rs          # 事件监听公共接口
│   ├── rdev.rs         # Windows/Linux 使用 rdev
│   └── macos.rs        # macOS 原生 CGEventTap
├── window/
│   ├── mod.rs
│   ├── windows.rs
│   └── macos.rs
└── pixel/
    ├── mod.rs
    ├── windows.rs
    └── macos.rs
```

**优点**：
- 每个平台实现隔离在独立文件
- 更容易添加 Linux 支持
- 代码审查时清晰看到平台差异

**缺点**：
- 需要较大重构
- 当前架构已经工作良好

### 方案 B：保持现状 + 渐进改进（当前选择）

保持现有文件结构，仅做以下改进：

1. ✅ 已完成：macOS 原生事件监听（`macos_events.rs`）
2. 待完成：补充 macOS 窗口 API（`window.rs`）
3. 文档：保持 `PROJECT_STRUCTURE.md` 更新

## 未来考虑

### Linux 支持

如果未来需要支持 Linux：

1. `rdev` 在 Linux 上工作良好（X11/Wayland）
2. 窗口 API 需要新实现（X11 或 Wayland 协议）
3. 像素读取需要新实现

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
| 2024-12 | 保持现有架构 | 当前架构满足需求，无需大规模重构 |

## 结论

**当前架构评级：✅ 良好**

不建议进行大规模重构。如果未来需要添加 Linux 支持或插件系统，再考虑方案 A 的重组。

