# Phase 3 - Conditions（条件与识别）

## 阶段目标

> **让宏能"看"和"判断"：基于窗口状态、像素颜色、计数器等条件执行不同分支。**

这是 tap 从"回放器"进化为"智能宏"的关键阶段：引入条件判断和简单分支，让宏能应对变化的场景。

## 前置条件

- Phase 2 已完成，录制/回放稳定

## 功能范围

### Must（必做）

| 功能 | 说明 | 验收标准 |
|------|------|----------|
| **目标窗口绑定** | 宏绑定到特定窗口（标题/进程名） | 窗口不匹配时暂停并提示 |
| **窗口状态检测** | 检查目标窗口是否前台/存在 | 支持 Win + mac |
| **像素颜色检测** | 检测指定坐标的颜色是否匹配 | 支持 RGB 容差 |
| **简单条件动作** | if (condition) then action | 单层 if/else |
| **变量与计数器** | 内置计数器，支持 incr/decr/reset | 用于循环控制 |
| **退出条件** | 当条件满足时停止宏 | 支持计数器/颜色/窗口 |
| **超时保护** | 等待条件超时后执行备选动作或停止 | 可配置超时时间 |

### Should（强烈建议）

| 功能 | 说明 | 理由 |
|------|------|------|
| **相对坐标** | 坐标相对于目标窗口而非屏幕 | 跨分辨率稳定性 |
| **颜色采样工具** | 在 UI 中点击获取颜色值 | 便于配置条件 |
| **条件可视化** | 在 Timeline 中显示条件节点 | 便于理解流程 |
| **等待直到（Wait Until）** | 等待条件满足后继续 | 常用模式 |

### Could（锦上添花）

| 功能 | 说明 |
|------|------|
| 图像匹配（模板匹配） | 检测某区域是否包含指定图像 |
| 多条件组合（AND/OR） | 复合条件 |
| 简单循环（for/while） | 结构化循环 |
| ROI 区域定义 | 限定检测/操作区域 |

### Won't（不做）

| 功能 | 理由 |
|------|------|
| OCR 文字识别 | 可作为 Phase 4 插件 |
| 复杂流程编排（子程序调用） | 可作为 Phase 4 扩展 |
| AI/ML 识别 | 超出 tap 定位 |

## 技术要点

### 窗口 API

- Windows: `FindWindow`, `GetForegroundWindow`, `GetWindowText`
- macOS: `CGWindowListCopyWindowInfo`, `NSRunningApplication`
- 封装到 `tap-platform` crate

### 像素颜色检测

- 使用屏幕截图 + 采样（避免频繁全屏截图）
- 容差计算：`|r1-r2| + |g1-g2| + |b1-b2| <= tolerance`
- 可选：使用系统 API 直接读取像素（更快但兼容性不同）

### 条件数据结构

```rust
pub enum Condition {
    WindowFocused { title: Option<String>, process: Option<String> },
    WindowExists { title: Option<String>, process: Option<String> },
    PixelColor { x: i32, y: i32, color: Color, tolerance: u8 },
    Counter { key: String, op: CompareOp, value: i32 },
    Always,
    Never,
}

pub enum CompareOp { Eq, Ne, Gt, Lt, Gte, Lte }

pub struct ConditionalAction {
    pub condition: Condition,
    pub then_action: Action,
    pub else_action: Option<Action>,
    pub timeout_ms: Option<u64>,
}
```

### 执行引擎扩展

- 在执行动作前评估条件
- 条件不满足时：等待/跳过/执行备选
- 超时后触发备选动作或停止

## UI 结构（扩展 Phase 2）

```
┌──────────────────────────────────────────────────────────────┐
│ Topbar: tap | Profile: [My Macro ▾] | Target: [Notepad ▾]   │
├────────────────┬─────────────────────────────────────────────┤
│ 左栏           │ 中栏: Timeline Editor                       │
│ - Profiles     │ ┌────────────────────────────────────────┐  │
│ - 条件模板     │ │ 0ms   Click @ (640, 360)               │  │
│                │ │ 500ms Wait Until: Pixel(100,200)=#FFF  │  │
│                │ │ ---   If counter < 10:                 │  │
│                │ │         Click @ (700, 400)             │  │
│                │ │       Else:                            │  │
│                │ │         Quit                           │  │
│                │ └────────────────────────────────────────┘  │
├────────────────┼─────────────────────────────────────────────┤
│ Inspector      │ 条件编辑区:                                 │
│ - 坐标拾取     │  - 条件类型: [Pixel Color ▾]                │
│ - 颜色采样     │  - 坐标: (100, 200)                         │
│ - 窗口列表     │  - 颜色: #FFFFFF  容差: 10                  │
├────────────────┴─────────────────────────────────────────────┤
│ Statusbar: Target: Notepad | Counter: 3 | ⚠️ Ctrl+Shift+Backspace │
└──────────────────────────────────────────────────────────────┘
```

## 验收标准

- [x] **窗口绑定**：能绑定到指定窗口，不匹配时正确暂停
- [x] **像素检测**：能检测指定位置颜色，容差正确
- [x] **条件执行**：条件满足/不满足时执行正确分支
- [x] **超时保护**：等待条件超时后正确处理
- [x] **计数器**：能正确增减和比较
- [x] **可停止**：条件等待中也能响应紧急停止

## 里程碑

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| M3.1 | 窗口 API 封装（Win + mac） | ✅ 已完成 |
| M3.2 | 像素颜色检测 | ✅ 已完成 |
| M3.3 | 条件数据结构 + 评估器 | ✅ 已完成 |
| M3.4 | 变量/计数器系统 | ✅ 已完成 |
| M3.5 | 执行引擎扩展（条件评估 + WaitUntil + 超时） | ✅ 已完成 |
| M3.6 | 相对坐标支持 | ⚠️ API 就绪，Action 集成待完善 |
| M3.7 | Tauri 命令层（窗口列表/颜色采样） | ✅ 已完成 |
| M3.8 | 前端 UI（窗口选择/颜色采样） | ✅ 已完成 |
| M3.9 | 文档更新 | ✅ 已完成 |

> **M3.6 说明**：`WindowRect::to_absolute()` API 已实现，可用于将相对坐标转换为绝对坐标。但 Action 本身尚未支持 `relative_to_window` 字段，当前需手动在执行前转换。完整的相对坐标支持可作为后续增强。


