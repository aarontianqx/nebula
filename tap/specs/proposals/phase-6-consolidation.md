# Phase 6 - Consolidation（让 tap 真正可用）

> **一句话目标**：把 Phase 1–4 已经"宣布完成"但实际断裂的能力**真正打通并跑通**，在 macOS / Windows 双平台达到一致可用，并把架构与前端重构到可长期维护的形态。Wasm 插件顺延至 Phase 7。

## 0. 为什么需要这个阶段

Roadmap 把 Phase 1–5 标为"完成 / 评估完成"，但代码审查发现核心卖点在执行路径上是断的：

- **变量 / 表达式不生效**：`{{ var }}`、`{{ base_x + 1 }}` 在 DSL 能解析，但 `Player` 执行时不替换（`engine.rs:254` 仅 `variables.clear()`，注入走 `engine.rs:545-559` 的原始 action）。
- **运行期 `Profile` 模型是有损的**：`Profile`（`lib.rs:49-57`）没有 `variables` / 元数据字段；`Profile::try_from`（`dsl.rs:596-623`）直接**丢弃** `dsl.variables`。这是 #1/#2/#14 的共同根因。
- **前端编辑不落库**：全代码库**从不调用 `update_profile`**。Timeline 的增删 / 禁用 / 调时、目标窗口绑定都只改本地 React state；`cmd_save_profile` 保存的是后端那份**未被编辑**的旧副本。
- **macOS 平台能力缺失**：window / pixel / dpi 全是 `None` / 硬编码 stub（你本机就是 macOS，意味着 Phase 3 在开发机上完全不可用）。
- **架构与 `AGENTS.md` 不符**：没有独立 Application 层；`Player`/`Recorder` 混在 `tap-core`；`main.rs:800-846` 实际承担了 Coordinator / EventBus / 状态机。
- **集成测试为零**：`engine.rs`、命令层、前端均无测试（单元测试只覆盖 `variables`/`expression`/`submacro`/`dsl` 的孤立逻辑），所以"测试通过"掩盖了"集成断裂"。

本阶段的判定标准很直接：**文档里写出来的每一个 DSL / UI 能力，都必须能在双平台真实执行；前端的每一次编辑都必须能保存并回放。**

### 范围（Scope）

- **In**：统一文档模型、执行期变量 / 表达式 / 子宏、双平台条件能力、录制降噪、拖拽插值与安全硬化、前端 Zustand+组件化重构、Timeline 编辑器、Profile/模板/运行前参数表单、权限与 Onboarding、Key→Click 补全。
- **Out（顺延 Phase 7）**：Wasm 插件系统、远程模板市场、云同步、OCR、多轨时间线高级编排。

### 贯穿原则（来自 `ui-design.md`）

可控 → 可观测 → 可预期 → 可恢复。任何重构都不得削弱"全局紧急停止随时生效"。

---

## 1. 目标架构

### 1.1 分层（对齐 `AGENTS.md`）

```
Adapter (src-tauri)         仅 IPC command + Tauri 装配，无业务逻辑
  ↓ 调用
Application (tap-application) Coordinator / Player / Recorder / EventBus / ToolModes / SessionStore
  ↓ 通过 trait 端口
Infrastructure (tap-platform) Injector / InputHook / Window / Pixel / DPI / Storage I/O
  ↓ 依赖
Domain (tap-core)            纯模型：MacroDocument / Action / Condition / Variables / Expression / DSL / Schema
```

**关键动作**：

- 新建 `crates/tap-application`：把 `Player`、`Recorder`、`EngineCommand/Event`、新增的 `Coordinator`、`EventBus`、`KeyClickRunner` 等从 `tap-core` / `src-tauri` 迁入。
- `tap-core` 回归**纯 Domain**：模型、DSL 序列化、条件、变量、表达式、schema（无线程、无 I/O）。
- `storage` I/O 归入 Application/Infra（保留接口在 application，落盘细节可委托 platform）。
- `src-tauri` 收敛为**薄适配层**：command → application API 的转译 + 事件转发，不再 `new` 基础设施、不再持有状态机。

### 1.2 统一文档模型（根治 #1/#2/#14）

当前 `Profile`（运行期）与 `DslProfile`（YAML）双轨且转换有损。**收敛为单一规范模型 `MacroDocument`**：

```
MacroDocument {
  metadata: { name, description, author, tags, created_at, updated_at }
  variables: Map<String, VariableDefinition>   // 定义 + 默认值
  target_window: Option<TargetWindow>
  timeline: Vec<TimedAction>                    // TimedAction 已含 note
  run: RunConfig
}
```

- 该模型是 Domain / 执行 / IPC / 存储的**唯一真相**。
- 序列化：统一走 YAML（DSL 形态）。`From`/`TryFrom` 必须**无损**，并加 round-trip property test。
- 存储迁移：落盘格式由 JSON → YAML（保留对旧 JSON 的兼容读取，一次性升级）。

### 1.3 执行管线引入 Resolve 阶段（根治 #1）

`Player` 持有一个 `VariableScope`（= 定义默认值 + 运行前覆盖 + 运行期 counter），并在**注入前**对每个 action 求值：

```
for action in timeline:
    resolved = resolve(action, &scope)   // 仅当字段含 "{{" 才处理
    inject(resolved)                      // 注入纯字面量 action
```

- `resolve()` 复用现成的 `resolve_expressions()`（简单标识符直查，复杂表达式走 Rhai 沙箱；引擎限制已在 `expression.rs:24-30` 设好）。
- 性能：仅对含 `{{` 的字段求值；对每个表达式**缓存编译后的 AST**，避免热循环重复 parse。
- 错误：求值失败 → 发 `EngineEvent::Error` 并安全停止（不静默吞错，符合 `AGENTS.md`）。
- 运行前覆盖：`start_execution` 接收 `runtime_vars`，注入 `Player` 的 scope（替换当前写入死字段 `app_state.variables` 的 `cmd_set_runtime_variables`）。

### 1.4 前后端同步契约（根治 #9 的整类 bug）

**后端 Application 持有规范 `MacroDocument`（SessionStore）= 单一真相**：

- 前端编辑 → 调 `mutate_document` / `set_document`（替代从不存在的 `update_profile` 调用）。
- 后端 `emit("document-changed")` → 其它视图（Visual / Code / Inspector）保持一致。
- `Start` / `Save` 永远基于后端规范副本；前端本地 store 仅作即时响应镜像，编辑后 debounce 推送 + 启动 / 保存前强制推送。

这样"编辑不落库 / 保存旧副本"在结构上不可能再发生。

### 1.5 前端架构

- **Zustand stores**：`documentStore`（被编辑的宏）、`engineStore`（运行态 / 事件）、`recorderStore`、`toolStore`（Key→Click）、`permissionStore`、`settingsStore`。
- **组件拆分**（替代 1386 行单文件）：`Topbar`、`Sidebar`(Profiles/Templates)、`TimelineEditor`(List + Rail 双视图)、`Inspector`(选中动作参数编辑)、`RunPanel`(Controls + 实时进度)、`ActivityLog`、`modals/`(RunForm 变量表单 / SaveDialog / Permissions / Onboarding)、`Picker`。
- 引入 `@tauri-apps/plugin-dialog` + `plugin-fs` 做原生导入 / 导出（替代 Blob/hidden input）。

---

## 2. 工作分解与里程碑

> 排序原则：先打**地基 + 正确性**（让已建成的能力跑通），再补**双平台**，最后做**编辑器 / UX 体验**。`P0` = 阻塞性 / 正确性，`P1` = 核心体验，`P2` = 完善。

| 里程碑 | 内容 | 优先级 | 依赖 |
|---|---|---|---|
| **M0** 测试与 CI 闸门 | 引擎集成测试脚手架（记录式 `NoopInjector` + mock `PlatformConditionProvider`，覆盖 repeat / 控制动作不注入 / conditional 分支 / 紧急停止）；`clippy -D warnings` + `fmt --check` + `test` 入 CI（Rust 工作区：tap-core 于 Linux，tap-platform/tap-tauri 于 macOS）。前端 store 单测随 M8 引入 store 时落地 | P0 | — |
| **M1** 统一文档模型 + 存储 | `MacroDocument` 收敛；无损 round-trip；YAML 落盘 + 旧 JSON 兼容读取 | P0 | M0 |
| **M2** Application 层抽离 | 新建 `tap-application`，迁移 Player/Recorder/EventBus，新增 Coordinator/SessionStore；`src-tauri` 薄化 | P0 | M1 |
| **M3** 执行期变量 / 表达式 | Resolve 阶段；`VariableScope`；运行前覆盖打通到 Player | P0 | M1,M2 |
| **M4** 子宏调用 `call_macro` | 新增 `Action::CallMacro`；引擎展开 + 复用 `submacro.rs` 的环 / 深度保护；child scope | P1 | M3 |
| **M5** macOS 平台对齐 | window（CGWindowList + AX 焦点）、pixel（ScreenCaptureKit，回退 CGWindowListCreateImage）、NSScreen `backingScaleFactor`；统一坐标系 + 修 Picker | P0 | M2 |
| **M6** 录制降噪 | 合并连续 / 共线 move；合成 Click/DoubleClick/KeyTap/Drag；可选窗口相对坐标 | P1 | M1 |
| **M7** 拖拽插值 + 安全硬化 | `Drag` 按 `duration_ms` 插值且可中断；sub-action cancel token；watchdog 超时；连续注入失败 N 次自动停；arming 子秒粒度 | P1 | M2 |
| **M8** 前端重构 + Timeline 编辑器 | Zustand + 组件化；List + Rail 双视图；Inspector 改参数；增 / 插 / 删 / 上下移 / 复制；拖拽调时；批量调延时；note 显示编辑；同步契约接入 | P1 | M1,M2,M3 |
| **M9** Profile / 模板 / 运行前表单 | 模板经 `resources` 打包 + 浏览 / 应用；最近使用；删除；元数据（描述 / 作者 / 标签）；变量运行前表单接入执行；原生 dialog 导入导出 | P1 | M1,M8 |
| **M10** 权限 + Onboarding | macOS 辅助功能 / 屏幕录制检测 + 引导 + 缺失时禁用录制 / 回放并说明；Windows 管理员提示；首次使用引导（紧急停止 + 示例宏） | P0 | M5,M8 |
| **M11** Key→Click 补全 | 鼠标按钮 Left/Right/Middle；位置 Cursor/Fixed；可选仅目标窗口前台；`min_interval_ms`（默认 40）命名 / 默认对齐；UI 选项 + 可观测文案 | P1 | M2,M8 |
| **M12** 杂项与收尾 | Simple Click 按钮可选 + 键捕获辅助；`NoopInjector` 暴露为"安全 / dry-run"开关；跨平台键名 normalize；修 `Picker.tsx:87` 乱码；文档与 roadmap 状态修订 | P2 | 各项之后 |

**可并行**：M5（平台）与 M3/M4（执行）可并行；M6（录制）与 M8（前端）可并行；M0 始终先行并持续。

---

## 3. 关键设计决策（详解）

### 3.1 变量 / 表达式语义

- 解析时机：**注入前**，每次迭代实时求值（counter 在循环中变化，必须每次重算）。
- 类型：坐标字段求值后须能转 i32（`resolve_to_i32`），失败即报错停止；文本字段保持字符串。
- 作用域优先级：运行前覆盖 > 变量定义默认值；counter 与变量同名时以变量为先（沿用现有 `resolve_expressions` 规则并补测试）。
- 安全：沿用 Rhai 沙箱（无文件 / 网络、深度 / 操作数上限）。

### 3.2 macOS 平台与权限矩阵

| 能力 | macOS API | 所需权限 |
|---|---|---|
| 输入注入 / 全局 hook | CGEvent / 现有 events 单例 | 辅助功能 (Accessibility) |
| 前台窗口 / 窗口列表 | CGWindowList + AXUIElement | 辅助功能（标题需屏幕录制） |
| 像素取色 | ScreenCaptureKit（回退 CGWindowListCreateImage） | 屏幕录制 (Screen Recording) |
| DPI scale | `NSScreen.backingScaleFactor`（按显示器） | 无 |

- 权限检测在 Application 层提供统一查询；前端 `permissionStore` 轮询 / 监听并据此 gate 按钮。
- 多显示器：scale 必须**按窗口所在显示器**取，不能用 primary。

### 3.3 坐标系统一（修 #12）

- 全链路统一为**全屏物理像素**。
- Picker：展示值与发送值必须一致——直接用后端 `get_primary_scale_factor()`（mac 接入真实 `backingScaleFactor` 后）换算，或让 overlay 直接上报物理坐标；录制 / 注入 / 拾取三者坐标系一致并加跨缩放回归测试。

### 3.4 安全模型（强化 #8）

- **可中断注入**：长动作（Drag 插值、未来长按）按小步推进，每步查 `should_stop`。
- **Watchdog**：单动作 / 单次运行超时 → 自动停 + 提示。
- **阈值**：连续注入失败 N 次自动停（`ui-design.md` 要求）。
- **可恢复**：异常 / 崩溃后提示"上次运行中断"，可查日志。
- 紧急停止热键路径保持最高优先级，重构中以测试守护。

### 3.5 测试策略（补 #集成空白）

- **引擎集成测试**：用 `NoopInjector`（记录注入序列）+ mock `PlatformConditionProvider`，断言"变量替换后注入的字面量正确""call_macro 展开正确""紧急停止能中断"。
- **模型 round-trip**：`MacroDocument` ↔ YAML 无损 property test。
- **前端**：store reducer / 同步契约单测。
- CI 阻断：`clippy -D warnings`、`fmt --check`、`cargo test`、`tsc`、eslint。

---

## 4. 验收标准（Definition of Done）

本阶段交付必须满足：

- [ ] DSL 文档中描述的每个能力都能**真实执行**：`{{ var }}`、`{{ expr }}`、`call_macro`、counter、conditional、wait_until。
- [ ] macOS 与 Windows 在**窗口绑定 / 像素条件 / DPI**上行为一致。
- [ ] 前端任意 Timeline 编辑（增删 / 调时 / 禁用 / 改参 / note）**保存后回放生效**；目标窗口绑定生效。
- [ ] 变量运行前表单可达且填入值**真实影响**执行。
- [ ] 模板可在打包后的应用内浏览并应用；最近使用 / 删除 / 元数据可用；导入导出走原生 dialog。
- [ ] 权限缺失时录制 / 回放被禁用并给出"去设置"引导；首次使用有 Onboarding。
- [ ] Key→Click 支持按钮 / 位置 / 速率配置且可观测。
- [ ] 紧急停止在双平台随时生效；存在 watchdog / 失败阈值自动停。
- [ ] `engine` 有集成测试；CI 全绿（clippy/fmt/test/tsc/eslint）。
- [ ] roadmap / phase 文档状态与现实一致。

**演示场景（E2E）**：导入带变量的"Auto Login"YAML → 运行前表单填用户名 → 选目标窗口 → 倒计时 → 按变量坐标 / 文本执行 → 运行中显示"第 X/Y 步 + 下一步 + 剩余时间" → `Space`/热键随时停。

---

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| ScreenCaptureKit 复杂 / 版本差异 | 先用 CGWindowListCreateImage 打底，SCKit 作增强；最低系统版本评估 |
| 重构 churn 大、易回归 | M0 先建测试网；分里程碑小步合并；紧急停止 / 注入路径优先加测试 |
| Rhai 在热循环性能 | 仅对含 `{{` 字段求值 + 缓存 AST |
| macOS 权限被拒导致功能"看似坏了" | 权限检测 + 明确引导 + 按钮 gating（M10 与平台能力同档期） |
| 范围蔓延 | 插件 / 远程市场 / 云同步明确顺延 Phase 7 |

---

## 6. 文档与路线图修订（随交付同步）

- `roadmap.md`：修正状态表——变量替换 / call_macro / macOS 由"完成"改为本阶段"进行中→完成"；插件移至 Phase 7。
- `phase-4-extensibility.md`：勘误自相矛盾的验收项（"变量替换✅" vs 待完成项）。
- `dsl-reference.md`：能力落地后移除 `call_macro` 的"待实现"标注，确认变量 / 表达式示例可用。
- `AGENTS.md`：架构图与实际分层一致后，确认 Application 层落点（`tap-application`）。
- `README.md`：补充权限 / 首次使用说明。

---

## 7. 附：根因 → 里程碑 对照

| 审查发现 | 根因 | 解决里程碑 |
|---|---|---|
| #1 变量 / 表达式不生效 | 有损模型 + 无 Resolve 阶段 | M1, M3 |
| #2 call_macro 无实现 | 无 Action 变体 + 引擎未集成 | M4 |
| #3 macOS 平台缺失 | 平台 stub | M5 |
| #4 录制噪音大 | 无语义合并 | M6 |
| #5 Drag 忽略时长 | 注入未插值 | M7 |
| #6 Key→Click 不完整 | 配置 / UI 缺项 | M11 |
| #7 缺 Application 层 | 分层混淆 | M2 |
| #8 紧急停止粒度 | 无 cancel token / watchdog | M7 |
| #9 前端不落库 / 状态混乱 | 无同步契约 + 无 Zustand | M2(契约), M8 |
| #10 Profile / 模板 / 最近使用 | 命令未接 + 模板未打包 | M9 |
| #11 Timeline 编辑器弱 | 仅列表视图 | M8 |
| #12 Picker 坐标不一致 | 坐标系未统一 | M5 |
| #13 权限 UI 缺失 | 无检测 / 引导 | M10 |
| #14 杂项（元数据 / 按钮 / NoopInjector） | 有损模型 + 硬编码 | M1, M12 |
