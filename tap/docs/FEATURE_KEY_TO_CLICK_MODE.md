# tap - 功能设计：Key→Click（A–Z 按住连点，Space 停止）

## 背景与动机（产品视角）

tap 当前在 “Simple” 模式下提供两类基础重复能力：

- 固定坐标的重复点击（Timer 驱动）
- 固定按键的重复按键（Timer 驱动）

但在实际使用中，“重复点击”常见痛点是：

- **鼠标连点疲劳**：需要持续/高频点击时，手部负担大
- **连续点击更难控**：鼠标在移动/切换焦点时容易误点
- **想要“按住就刷、松开就停”**：键盘天然支持按住触发的肌肉记忆

因此引入一个轻量“即时工具模式（Tool Mode）”：

> 开启后，用户按住键盘 A–Z 任意键即可持续模拟鼠标点击；按下 Space 立即终止（并退出该模式）。

它的定位是：**快速替代鼠标重复点击**，并与 “Timeline/DSL 宏”互补，而不是替代。

## 用户故事（User Stories）

- 作为用户，我想把鼠标放在目标按钮上，然后**按住任意字母键就不断点击**，松开就停，避免鼠标连点疲劳。
- 作为用户，我希望在任何时候都能**按空格立刻终止**，防止失控。
- 作为用户，我希望在启用该模式时，UI 明确告诉我它正在捕获全局按键、当前配置是什么、怎么停止。

## 功能定义（What / Non-Goals）

### What（本功能做什么）

- **触发方式**：启用模式后，按住 `A`–`Z` 任意键 → 持续产生鼠标点击。
- **终止方式**：
  - 按下 `Space`：立即停止，并退出 Key→Click 模式（回到 Idle）
  - 或点击 UI 中的 Stop（等价于停止并退出模式）
  - 全局紧急停止热键（`Ctrl+Shift+Backspace`）仍然随时生效
- **点击行为**：
  - 默认：在**当前鼠标指针位置**执行点击（用户把鼠标放在目标处即可）
  - 支持选择点击按钮：Left / Right / Middle（默认 Left）
- **节流/速率限制**：
  - 提供 `min_interval_ms`（例如默认 30–50ms）避免过快导致系统卡顿或目标程序异常
  - “按住”期间以固定间隔点击（而不是依赖 OS 的键盘 repeat 频率）

### Non-Goals（本功能不做什么）

- 不把 Key→Click 直接做成 DSL 动作（DSL 当前是 timed actions；Key→Click 是 event-driven）
- 不支持任意键映射（本版固定为 A–Z，避免误触功能键/系统快捷键）
- 不做“多键并发不同动作”（多键同时按下按单一规则处理，见下文）

## UI/UX 设计（如何放进现有 UI，并保持协调）

### 入口：Simple → Action 增加第三项

在 `tap/src/App.tsx` 的 Simple 配置里目前有：

- Action: Click / Key Press

建议改为：

- Action: Click / Key Press / **Key→Click**

这样符合 “从简单到强大” 的渐进逻辑，也符合现有侧边栏卡片结构，不引入新的顶层 Tab。

### 配置项（Key→Click 模式）

当 Action=Key→Click 时，侧边栏展示：

- **Trigger keys**：`A–Z`（只读提示）
- **Stop key**：`Space`（只读提示，强调“立即终止”）
- **Mouse button**：Left / Right / Middle（默认 Left）
- **Click location**：
  - Cursor（默认）
  - Fixed position（可选）：复用现有 Pick，填入 X/Y
- **Rate limit**：`min_interval_ms`（默认 40ms，范围建议 10–1000ms）
- （可选增强）**Only when target window focused**：复用 Phase 3 的窗口绑定（若启用，则仅在目标窗口为前台时触发点击）

### 运行态提示（避免“悄悄捕获键盘”）

Key→Click 运行时属于“高风险输入工具”，必须“可观测”：

- Topbar / Statusbar 显示 **Key→Click Active**
- 主控区的 Stop 按钮保持显眼
- Statusbar 固定提示：`Hold A–Z to click | Space to stop`
- Activity Log 记录：
  - Started Key→Click (button=Left, location=Cursor, min_interval=40ms)
  - Stopped by Space / UI Stop / Emergency Stop

### 与现有风格的协调

现有 UI 已经有：

- `Simple/Timeline` 两个 Tab
- `Controls` 卡片统一承载 Play/Pause/Stop
- `Safety` 卡片强调紧急停止热键

Key→Click 只需要：

- 在 Simple 的配置卡片里增加一个 action type
- 在 Running 状态文案与日志中“说清楚正在做什么”
- 不新增复杂编辑器、不过度引入新视觉组件

## 交互细节与边界条件

### A–Z 的定义

- 以 `tap-platform::InputEventType::KeyDown/KeyUp { key: String }` 的 key 名称为准
- 认为 `A`–`Z`（大写）是有效触发键；如平台返回小写，则统一 normalize

### 多键按住规则（简化但可解释）

建议规则（简单且直觉）：

- 任意时刻只允许一个“active trigger key”
  - 如果当前没有 active key：第一个按下的 A–Z 成为 active，开始连点
  - 如果已有 active key：其他 A–Z 按下忽略（避免不可控）
  - active key 松开：停止连点，回到 “Armed”（等待下一次 A–Z 按下）

### Stop key（Space）的优先级

- Space 的 KeyDown 一律立即终止并退出模式
- Space 必须高优先级处理，即使当前正在连点也要立刻停止

### 与录制/回放的互斥

为了避免多个全局 hook 并存导致复杂度上升，建议 MVP 约束：

- Key→Click 只能在 Engine=Idle 且 Recorder=Idle 时启动
- 启动后，Engine 状态进入 Running（或引入 ToolState；见架构建议）

## 架构设计（跨平台与代码组织）

### 为什么它不是 DSL Action

DSL 的抽象是 “在时间线上的动作”，而 Key→Click 是：

- 由用户输入（键盘按住/松开）实时驱动
- 持续时间不确定
- 触发频率与用户行为相关

强行塞入 DSL 会把 Engine 复杂化（需要 event loop、输入订阅、与 timeline 调度融合），收益不高。

因此建议它属于 **Application 层的 Tool Mode**：在 Tauri 后端启动一个运行器，订阅全局输入事件，调用 injector 产生点击，并向前端 emit 状态事件。

### 推荐分层落点

- `tap-platform`：继续承担 **InputHook（全局键盘事件）** 与 **Injector（鼠标注入）**
- `tap-core`：保持 “Timeline/DSL/条件” 的纯模型与播放器逻辑，不强行新增 event-driven action
- `tap/src-tauri`：新增一个 **ToolRunner（KeyClickRunner）**，管理线程、stop token、事件上报

这样符合 `docs/PROJECT_STRUCTURE.md` 的分层原则：平台差异留在 platform，核心引擎留在 core，产品级编排留在 application/tauri。

### 关键实现接口（建议）

在 `tap/src-tauri/src/main.rs` 增加命令（示例命名）：

- `cmd_start_key_click_mode(config)`：启动模式
- `cmd_stop_key_click_mode(reason)`：停止模式（UI Stop / Space / EmergencyStop）
- 通过 `app.emit("tool-event", ...)` 向前端发送：
  - `ToolStateChanged`
  - `KeyClickActiveKeyChanged`
  - `KeyClickClickCount`（可选）

配置结构（概念）：

- `mouse_button: Left|Right|Middle`
- `location_mode: Cursor | Fixed(x,y)`
- `min_interval_ms: u64`
- `stop_key: Space`（固定）
- `allowed_keys: A–Z`（固定）

### 与现有紧急停止的关系

紧急停止有两条路径：

1) 全局热键（`Ctrl+Shift+Backspace`）触发 `EngineCommand::EmergencyStop`
2) UI 按钮调用 `emergency_stop`

Key→Click 需要保证：

- 触发紧急停止时，**无论 Key→Click 是否在运行，都要立即停止**
- 实现上可以在 `handle_emergency_stop` 中同时触发 `stop_key_click_mode`（或 broadcast 一个全局 stop token）

## 权限与平台差异

- macOS：Key→Click 依赖全局键盘事件监听与输入注入，需辅助功能权限；UI 需要沿用已有权限提示策略
- Windows：部分目标应用可能需要管理员权限；沿用既有提示（必要时“以管理员运行 tap”）

## 文档更新建议（本文件之外）

- `docs/FUNCTIONAL_GUIDE.md`：在 “重复点击/重复按键” 下补充 Key→Click 工具模式
- `docs/UI_DESIGN.md`：在 Onboarding 与 Safety/Running 状态提示中加入 Key→Click 的可观测要求
- `docs/DSL_REFERENCE.md`：明确“event-driven 工具模式不属于 DSL”，避免用户误以为可以 YAML 配置实现
- `docs/roadmap/ROADMAP.md`：把 Key→Click 作为 “跨阶段体验增强 / Quick Tools” 记录，避免散落在 issue/脑内


