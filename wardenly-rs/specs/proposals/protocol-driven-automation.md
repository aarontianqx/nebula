# Proposal: 协议驱动自动化落地方案

> 2026-09-02 ｜ 状态：Phase 1–3 已实现（登录链路直达、协议桥、协议脚本引擎；live 集成测试 `src-tauri/tests/{login_chain,protocol_bridge,protocol_script}.rs`）｜ 前置文档：[protocol-automation-recon.md](protocol-automation-recon.md)（调研事实与验证记录，本文档不再重复）

## 1. 背景与决策

调研已验证：WLY 的协议层完全暴露在页面 JS 侧，注入页面后可直接调用游戏自己的 `Connection` 收发协议，无需截图识别、无需模拟点击、无需触碰加密。AES 密钥为 bundle 内硬编码字面量，不随登录变化，仅随游戏版本更新漂移。

本文档做出决策并固化落地计划：**采纳调研文档 §7.2 的形态 2 —— 集成进 wardenly-rs 受管浏览器**。

- 保留现有 session 管理、登录、画布预览能力；
- 新增协议驱动层：通过 CDP 向游戏页注入 JS 桥，自动化从"截图 → 场景识别 → 模拟点击"升级为"协议编排 + 结构化状态"；
- 截图 + 场景识别 + OCR 不删除，降级为兜底路径（首次用户协议、意外弹窗、协议未覆盖的场景）。

选择形态 2 而非形态 1（Tampermonkey）/ 形态 3（脱机客户端）的理由：

- 形态 1 依赖用户自己开浏览器，与 wardenly-rs 的多 session 管理定位冲突；
- 形态 3 需自行实现 WS 客户端与登录票据链逆向，工程量与维护成本最高，且失去画布预览这一核心产品能力；
- 形态 2 与现有架构（每 session 一个 CDP 实例 + actor 模型）天然兼容，改动集中在 seams 已知的几个点。

## 2. 目标与非目标

### 目标

1. 登录链路全 DOM 化：layer1 填表 → layer2 → 直达 layer3 游戏页，消除对像素匹配登录检测的依赖；
2. 每个 session 内：画布预览（截图）与协议驱动（页面 JS 桥）在同一 CDP 会话中并行工作；
3. 自动化脚本可通过协议收发完成，结构化下行数据替代 OCR 做状态判断。

### 非目标（本期不做）

- 不做脱机客户端（形态 3），不自行实现 WS 协议栈；
- 不逆向登录票据链（ticket / content 票据均由平台页面现取现用）；
- 不删除现有场景识别 / OCR / 模拟点击能力；
- 不做多开、不做规模化（合规约束，见调研文档 §7.3）。

## 3. 总体架构

```
┌─ Adapter ─────────────────────────────────────────────┐
│  Tauri commands: 复用现有；ProtocolEvent 经 EventBus 推前端 │
├─ Application ─────────────────────────────────────────┤
│  SessionActor: 登录流程改为 DOM 链路；新增协议命令处理      │
│  ProtocolRunner（新）: 协议编排脚本执行引擎，与 ScriptRunner 并存 │
│  GameState（新）: 下行协议聚合成的结构化游戏状态            │
├─ Infrastructure ──────────────────────────────────────┤
│  BrowserDriver 扩展: evaluate 已存在，新增 init-script 注入 │
│  PageBridge（新, JS）: 注入游戏页，patch _parsePacket 观测下行，│
│    暴露 send(protocolId, payload) 调用游戏 Connection     │
│  ProtocolRegistry（新）: 协议名/id/结构定义，从 bundle 提取   │
├─ Domain ──────────────────────────────────────────────┤
│  新增 Protocol 相关 model 与 DomainEvent；Scene/Script 不变   │
└───────────────────────────────────────────────────────┘
```

关键原则：

- **协议层不碰二进制与密码学**：组包、加密、解析全部由游戏自己的 `Connection` 代劳，JS 桥只做"调用 + 观测"；
- **预览与协议解耦**：截图预览（chromium.rs `start_screencast`，333ms 轮询）不依赖页面内部状态，与协议桥并行无冲突（已实测验证）；
- **页面直达是前提**：`evaluate` 只作用于顶层页面，因此必须先完成 Phase 1 的直达改造，协议桥才可用。

## 4. 分阶段实施

### Phase 1：登录链路直达游戏页

独立收益最大的一步：即使不做协议驱动，DOM 链路也比像素匹配稳定。

改动点：

- `src-tauri/src/infrastructure/browser/chromium.rs`
  - launch flags 增加 `--disable-features=HttpsUpgrades`（约 chromium.rs:208 `start()`；不加则直达 layer3 后 ws 被 mixed-content 拦死，游戏静默卡死）；
  - `BrowserDriver` trait（driver.rs:21）无需新增方法：现有 `navigate` / `evaluate` / `login_with_password` / `click` 足够。
- `src-tauri/src/application/service/session_actor.rs`
  - 重写 `perform_login`（约 :321）：
    1. navigate layer1（`http://www.lequ.com/server/wly/s/{server}/ish5/{server}`，注意补上 `/ish5/{server}` 后缀，当前 game_url() 没有）；
    2. `login_with_password` 填表提交；
    3. `evaluate` 读顶层 iframe src（layer2 URL，带 ticket）；
    4. navigate layer2，`evaluate` 读 `#gameIframe.src`（layer3 URL，content 票据短时效，必须现取）；
    5. navigate 直达 layer3；
    6. 等待 `Connection._connected === true` 且桥观测到登录数据推送结束（`S_2_C_CHAR_LOAD_END`）作为 Ready 判据（替代像素匹配；单一 `_connected` 不足以判定服务端已接受业务协议，见 Phase 3 实现纪要）；
  - 用户协议弹窗：保留现有场景识别 + 坐标点击作为兜底（首次登录出现，同意过后不再出现）。这是 Phase 1 中唯一保留像素路径的地方。
- 状态机不变（`Idle → Starting → LoggingIn → Ready`），仅 Ready 判据改变。

验收： fresh profile（无缓存登录态）从启动到进主城全自动完成；主城画面正常推流；`evaluate` 读到 `Connection._connected === true`。

### Phase 2：协议桥（Page Bridge）

改动点：

- `src-tauri/src/infrastructure/browser/`
  - 新增 init-script 注入能力：chromiumoxide 对应 `Page.addScriptToEvaluateOnNewDocument`；`BrowserDriver` trait 新增 `add_init_script(script)`，在 navigate layer3 之前调用（游戏建连很早，事后 eval 注入会错过握手，见调研 §6.4）；
  - `evaluate` 返回值用于桥握手与协议发送结果回读。
- 新增 JS 资产 `page_bridge.js`（嵌入资源，随 init script 注入）：
  - 等 `__require('Connection')` 可用后 patch `_parsePacket`，将全部下行包（id → 结构化 data）序列化推送到 Rust 侧；
  - 暴露 `window.__wardenly.send(protocolName, payload)`：查 `Protocol` 枚举 → `Connection.send()`；
  - 下行→Rust 的通道：优先 `Runtime.addBinding`（CDP binding 回调直达）；若 chromiumoxide 支持不佳，退化为页面内环形缓冲 + Rust 侧轮询 `evaluate` 拉取（心跳间隔 333ms 与截图同频即可，实现最简单）。
- `src-tauri/src/domain/event.rs`：新增 `DomainEvent::ProtocolMessage { session_id, protocol_id, name, payload }`。
- `src-tauri/src/application/command.rs`：新增 `SessionCommand::SendProtocol { name, payload }`。

验收： session Ready 后，Tauri 侧发 `C_2_S_MAIL_INFO`，前端/日志能看到结构化邮件列表事件；心跳包持续到达。

### Phase 3：协议驱动脚本引擎

- 新增 `ProtocolRunner`（application 层，与 `ScriptRunner` 并存）：脚本动作原语从 click/wait/drag 扩展为 `send_protocol` / `wait_protocol`（等某下行协议且字段满足条件）/ 保留 `wait`；画面原语（click 等）保留作兜底。
- `GameState`：订阅 `ProtocolMessage` 事件流，聚合成可查询的结构化状态（如邮件数、资源量），替代 OCR 条件判断。
- 脚本 DSL：新增协议类 step，YAML schema 向后兼容（`type: send_protocol` 等新 action 种类）。DSL 细节另行提案，不在本方案内展开。
- `ProtocolRegistry`：从游戏 bundle 提取协议名 → id 映射与字段结构，生成为 Rust 侧资源文件（构建期脚本或一次性生成 + 版本标记）。

验收： 选一个现有日常任务（如邮件一键领取），用纯协议脚本完成，全程无截图识别、无模拟点击。

#### Phase 3 实现纪要（2026-09-02，以代码为准）

- 协议脚本放在独立目录 `resources/protocols/*.yaml`（与场景脚本 schema 不同，不混用）；`start_script` 按名字先查场景脚本、再查协议脚本，两种脚本共用同一套 run_id / ScriptStarted / ScriptStopped 生命周期。
- DSL 动作原语：`send_protocol` / `wait_protocol` / **`request`（发送+等待+超时重发，首选）** / `wait_state` / `wait` / `click` / `drag`；条件为字段路径比较（`state.S_2_C_X.field op value`，op 含 `exists`）。
- `GameState` 由桥转发任务单写，按协议名存最新负载；step 级 `conditions` 直接查询。
- `ProtocolRegistry` 已生成（`resources/protocols/registry.json`，2642 个协议，标注 bundle 版本 `mobile_v614_1334`），用于脚本启动前的协议名校验。
- 实现中修正了两个方案时未预见的点：
  1. **Ready 判据不够**：WS 建连早、可能被用户协议弹窗挡住入城，`_connected` 为 true 时服务端仍不处理业务协议。现 Ready 判据 = `_connected` + 桥观测到 `S_2_C_CHAR_LOAD_END`（登录数据推送结束），协议弹窗点击在整个等待窗口内持续重试（自愈）。
  2. **服务端应答不总是专用协议**：`C_2_S_MAIL_DRAW_ALL_REWARD` 的确认视邮件内容为专用 ack 或通用资源推送 `S_2_C_UPDATE_BENEFIT`，故 `request` 支持 `expect_any` 多应答协议。
- 验收通过：`tests/protocol_script.rs` live 跑通 `claim_all_mail`（拉列表 → 一键领取），协议交换全程 <1s；登录 + 协议弹窗兜底 + 桥安装全链路 21.9s。

### Phase 4（后续，另行立项）

- 拟人化节奏（随机间隔、在线时长控制）——降低行为指纹风险；
- bundle 版本监控与协议漂移 diff 工具；
- 协议覆盖率提升，逐步收缩画面兜底路径的使用场景。

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| content 票据短时效（约 5 分钟） | 每次启动从 layer2 现取，不缓存；登录流程本身即保证新鲜 |
| 单会话顶号（`S_2_C_KICK_OUT`） | 自动化运行期间视为独占；收到踢下线协议时明确上报事件，不要静默重连顶回 |
| 协议/密钥随版本漂移 | `ProtocolRegistry` 带 bundle 版本号；漂移时重新下载 bundle 生成；JS 桥按协议名调用，id 漂移由枚举自查吸收 |
| 行为指纹检测 | 纯协议流量无 UI 事件；Phase 4 拟人化；仅限本人账号自用 |
| init script 注入过早导致桥失效 | 桥自身做 `Connection` 可用性等待与重试；注入失败时 Ready 判据不通过，显式报错而非静默 |
| https 升级坑回归 | launch flag 固化在 ChromiumDriver；加一条启动自检（navigate 后断言 `location.protocol === 'http:'`） |

## 6. 验证记录

2026-09-02 已在真实账号（888 区）上验证本方案依赖的全部关键事实：

- 登录链路三层均可通过 DOM 操作到达，`#gameIframe.src` 可读取；
- 直达 layer3 后 headless 下游戏正常进主城、截图预览正常；
- 运行时 `_sessionKey` 与 bundle 硬编码字面量一致（确认不随登录变化）；
- `Connection.send(C_2_S_MAIL_INFO)` 收到 49 封邮件结构化数据 + 列表 + 心跳，全程无点击、无解密；
- 协议收发与截图预览在同一 CDP 会话并行无冲突。
