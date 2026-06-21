# tap - 功能设计：Key→Click（A–Z 按住连点，Space 停止）

## 背景与动机

Simple 模式下的“重复点击”常见痛点：鼠标连点疲劳、移动/切焦时易误点、想要“按住就刷、松开就停”的肌肉记忆。

因此引入一个轻量的**即时工具模式（Tool Mode）**：

> 开启后，按住键盘 `A`–`Z` 任意键即可持续模拟鼠标点击；按下 `Space` 立即终止并退出该模式。

定位：**快速替代鼠标重复点击**，与 Timeline/DSL 宏互补，而非替代。

## 功能定义

### What

- **触发**：启用后按住 `A`–`Z` 任意键 → 持续产生鼠标点击（轻点=单击；按住超过 `hold_delay_ms` 后以最小间隔连点）。
- **终止**：`Space` 立即停止并退出模式；UI Stop 等价；全局紧急停止热键（`Ctrl+Shift+Backspace`）随时生效。
- **点击位置**：默认在当前鼠标指针位置；可选固定坐标（复用 Pick）。
- **鼠标按键**：Left / Right / Middle（默认 Left）。
- **限频**：`min_interval_ms`（默认 40ms，夹取 10–1000ms），“按住”期间以固定间隔点击，而非依赖 OS 键盘 repeat 频率。
- **窗口锁定（可选）**：“仅在启动时活动窗口内点击”——按下 Start 时快照当前活动窗口（优先按进程名匹配），之后仅当该窗口为前台才点击，alt-tab 离开即暂停、切回即恢复。

### Non-Goals

- 不把 Key→Click 做成 DSL 动作（DSL 是 timed actions；本功能是 event-driven）。
- 不支持任意键映射（固定 A–Z，避免误触功能键 / 系统快捷键）。
- 不做“多键并发不同动作”。

## 交互规则与边界

- **A–Z 判定**：以 `tap-platform` 的 `KeyDown/KeyUp { key }` 为准，按键名归一化后判断；只有 A–Z 触发。
- **多键规则**：任意时刻只允许一个 active trigger key——无 active 时第一个按下的 A–Z 成为 active 并开始连点；已有 active 时其余 A–Z 按下忽略；active 松开则停止连点回到 Armed（等待下一次按下）。
- **Space 优先级**：`Space` 的 KeyDown 一律最高优先级，即使正在连点也立即停止并退出。
- **与录制/回放互斥**：仅在 Engine=Idle 且 Recorder=Idle 时可启动，避免多个全局 hook 并存。
- **可观测（高风险输入工具必须做到）**：状态栏常驻 `Hold A–Z to click | Space to stop`，配置卡显示实时点击计数；Activity Log 记录启动参数与停止原因（Space / UI Stop / Emergency Stop）。

## 设计取舍：为什么不是 DSL Action

DSL 的抽象是“时间线上的动作”，而 Key→Click 由用户实时输入驱动、持续时间不定、频率与用户行为相关。强行塞入会把引擎复杂化（需事件循环、输入订阅、与时间线调度融合），收益不高。

因此它属于 **Application / Tauri 层的 Tool Mode**：后端启动一个 Tool Runner，订阅全局输入事件、调用 injector 产生点击、向前端 emit 状态事件。这符合分层原则——平台差异留在 `tap-platform`，核心引擎留在 `tap-core`，产品级编排留在 application/tauri（见 `tap/AGENTS.md`）。

紧急停止需保证：无论 Key→Click 是否在运行，触发紧急停止都立即停止它。

## 权限与平台差异

- **macOS**：依赖全局键盘监听与输入注入，需「辅助功能」权限；沿用既有权限提示策略。
- **Windows**：目标应用可能需管理员权限，沿用“以管理员运行 tap”的提示。

## 实现落点

- 后端：`tap/src-tauri/src/key_click.rs`（Tool Runner 线程 + 输入订阅 + 注入），命令 `start_key_click` / `stop_key_click` / `get_key_click_status`。
- 前端：`toolStore` + `SimpleConfig`（Simple 模式下 Action 增加 Key→Click 项）。
- 行为与上文一致：触发 A–Z、Space/Stop 终止、与 Engine/Recorder 互斥，配置项含鼠标按键、点击位置、`min_interval_ms`、`hold_delay_ms`、窗口锁定。
