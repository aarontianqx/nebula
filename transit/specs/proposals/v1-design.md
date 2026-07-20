# Transit — v1 系统设计

> 状态：Implemented（本地验收通过）
> 首次起草：2026-07-19
> 本次修订：2026-07-19
> 适用范围：Transit v1 macOS 菜单栏 App 与 Widget

## 1. 背景

日常开发会同时使用多个 AI 编程 Agent。它们可能连接官方模型 API、兼容协议的第三方中转，或组织内部的统一网关。模型响应通常包含 token usage，但不同协议使用不同字段和流式事件：

- OpenAI Chat：`prompt_tokens` / `completion_tokens`；
- Chat-compatible streaming responses may report usage at the response root or under `choices[*].usage`;
- OpenAI Responses：`input_tokens` / `output_tokens`；
- Anthropic Messages：输入与输出 usage 分布在不同 SSE 事件中；
- cache、reasoning 等明细的命名和嵌套位置也不一致。

Agent 自带的遥测和本地日志无法稳定覆盖所有客户端；账户额度接口则是另一类问题，其查询地址、认证、周期和业务语义都由具体来源决定，无法由通用代理推断。

Transit v1 只解决一个核心问题：

> 让用户把 Agent 的协议流量显式路由经过本机，在不改变请求业务语义的前提下，从响应中提取协议级 usage，形成统一、可查询的本地用量事件，并通过菜单栏和桌面 Widget 展示。

Transit 不内置任何具体 Agent、模型服务商、中转站或账户额度来源。所有 route、上游、认证引用和价格均由用户在 App 中配置。

### 1.1 已验证事实

早期 PoC 已验证：

1. 对允许自定义 `base_url` 的客户端，本地反向代理可以透传普通 JSON 与 SSE 流式响应。
2. usage 可以在不记录 prompt、completion 正文的情况下从响应事件中提取。
3. 部分 managed OAuth 客户端会校验登录上下文，修改 `base_url` 后可能在发出请求前直接拒绝认证。
4. API key 或兼容网关模式更适合显式反向代理；网络层 MITM 需要安装 CA，安全与维护成本过高。

这些事实证明了产品方向可行，但不代表旧 PoC 的语言、并发模型和运行方式适合正式版本。

### 1.2 被替代的旧方案

早期的 `proxy.py` 与 `mitmproxy_addon.py` 是协议验证产物，不属于 v1 正式架构，现已从仓库删除：

- v1 不使用 Python `http.server`；
- v1 不使用独立 Go daemon；
- v1 不提供 agent-first CLI 和本地 Control API；
- v1 不使用 mitmproxy/MITM 覆盖 OAuth 流量；
- v1 不完整缓冲响应后再解析；
- v1 不承诺旧 PoC 的环境变量与输出格式兼容。

仓库只保留 Swift 实现，避免出现两套行为不同的代理。

## 2. 产品形态

Transit v1 是一个常驻菜单栏的原生 macOS App：

- App 启动时启动本地代理；
- App 运行期间持续接收 Agent 请求、提取 usage 并写入 SQLite；
- 菜单栏面板用于配置 route、查看统计与诊断；
- Widget 读取 App 写入的共享快照，不自行联网；
- 用户可以设置登录时自动启动 App。

v1 不拆分后台 daemon。菜单栏窗口关闭不影响代理，但用户退出整个 App 后代理立即停止。

这是一个明确的产品约束：如果 Agent 的 `base_url` 固定指向 Transit，而 Transit App 没有运行，该 Agent 请求将失败。App 必须通过登录启动、健康状态和退出提醒降低风险，但不能宣称进程级 fail-open。

## 3. 设计原则

1. **协议优先，不识别来源**

   内核只认识 `openai_chat`、`openai_responses`、`anthropic_messages` 等 wire protocol，不出现服务来源枚举、固定 endpoint 或内置账户配置。

2. **单体部署，模块化实现**

   Proxy、protocol parser、storage 和 UI 位于同一个 App 中，但通过清晰接口隔离。未来需要 headless/cross-platform 时可以抽出内核，不为尚未出现的需求提前引入多进程。

3. **语义透明，不宣称字节透明**

   Transit 正确处理 hop-by-hop header、Host、连接复用和内容编码，但不得修改请求 JSON、模型名、认证值或响应业务内容。

4. **转发优先，观测旁路**

   usage 解析、SQLite 和 Widget 更新失败不能中断已经建立的模型请求。统计队列满时允许丢弃事件并显式告警，不允许阻塞响应流。

5. **不持久化业务正文**

   不保存 prompt、completion、tool 参数或完整响应，只保存 route 元数据、HTTP 结果和已解析 usage 子对象。

6. **配置完全属于用户**

   默认配置为空。上游 URL、Agent 标签、认证引用和价格都由用户在运行时创建；源码、测试和二进制不携带任何真实来源预设。

7. **报告值与估算值不可混淆**

   上游返回的 usage 标记为 `reported`；无法获得时标记为 `missing`。成本只能来自用户价格规则，标记为 `estimated_cost`。

## 4. 目标与非目标

### 4.1 v1 目标

- 原生 macOS 菜单栏 App，支持登录自动启动。
- App 内运行一个或多个 loopback HTTP listener。
- 用户通过界面创建任意数量的 route。
- 支持路径前缀路由与独立端口路由。
- 支持普通 JSON 与 SSE 的增量透传和 usage 解析。
- 统一 OpenAI Chat、OpenAI Responses、Anthropic Messages 的 usage 字段。
- 每个完成、失败或取消的请求生成一条 `UsageEvent`。
- 使用 SQLite 保存事件，并按时间、route、agent、protocol、model 聚合。
- 支持认证透传与显式 credential injection。
- 通过菜单栏面板与 Widget 展示本机观测到的 usage。

### 4.2 v1 非目标

- 不轮询或计算账户余额、套餐额度、滚动窗口或重置周期。
- 不内置任何官方服务、中转站或内部服务的名称、URL、认证逻辑和价格表。
- 不提供 CLI、后台 daemon、Unix socket Control API 或 Web 管理页面。
- 不支持 Windows、Linux 和无界面服务器。
- 不通过本地 tokenizer 伪装成上游精确 usage。
- 不安装根证书，不做 TLS MITM，不绕过 managed OAuth 客户端保护。
- 不修改请求正文，不做模型映射、prompt 改写、响应缓存和自动重试。
- 不做多用户网关、计费运营、配额执行和访问控制平台。
- 不允许 Widget 直接联网、访问 SQLite、读取 secret 或控制代理。

### 4.3 未来扩展边界

如果未来需要展示外部账户额度，Transit 最多提供来源无关的 `QuotaSnapshot` 写入协议。具体查询机制必须由用户配置的外部 Collector 负责，不进入 Transit 源码和默认发行物。该能力不属于 v1。

如果未来出现 headless 或跨平台需求，再把 `TransitCore` 抽成独立进程；v1 的内部接口应允许拆分，但不提前承担进程通信、守护和双重签名成本。

## 5. 领域模型

Transit 拆开以下概念，不能用一个 `provider` 字段混合表达：

| 概念 | 含义 | 来源 |
| --- | --- | --- |
| `agent_id` | 用户为客户端实例设置的标签 | 用户配置 |
| `protocol` | 请求和 usage 遵循的 wire protocol | Transit 通用能力 |
| `route_id` | 本地入口到上游的转发规则 | 用户配置 |
| `upstream` | route 的目标 base URL | 用户配置 |
| `auth_policy` | 认证透传或注入方式 | Transit 通用能力 |
| `pricing_policy` | 可选价格规则 | 用户配置 |

同一个 Agent 可以切换多个 route；多个 route 可以使用同一种协议；协议相同也不代表上游价格和额度语义相同。

### 5.1 Protocol

v1 内置三个协议 Adapter：

- `openai_chat`
- `openai_responses`
- `anthropic_messages`

Adapter 只负责：

1. 识别请求与响应事件；
2. 尽力从请求或响应中提取 model；
3. 从 JSON 或 SSE 中提取 usage；
4. 映射到统一字段；
5. 保留最小化的原始 usage JSON；
6. 报告 `reported` 或 `missing`。

### 5.2 Route

Route 完全由用户创建，不存在内置 route：

```json
{
  "id": "primary-coding",
  "display_name": "Primary Coding",
  "agent_id": "coding-agent-01",
  "listener": {
    "port": 8787,
    "path_prefix": "/primary"
  },
  "upstream": "https://llm.example.com/v1",
  "protocol": "openai_responses",
  "auth": {
    "mode": "passthrough"
  },
  "pricing_policy_id": null,
  "enabled": true
}
```

规则：

- listener 只允许绑定 loopback；
- prefix 按完整 path segment 做最长匹配，`/a` 不匹配 `/abc`；
- 允许 `/a` 与 `/a/b` 嵌套，通过最长匹配确定 route；
- 命中后剥离 prefix，再与 upstream path 规范化拼接；
- query string 原样转发但不持久化；
- 未命中返回 404，不转发到默认上游；
- 不保留 base URL path 的客户端可使用独立端口 route；
- route 修改必须先校验，成功后原子替换运行中配置。

### 5.3 Authentication policy

v1 支持：

- `passthrough`：保留客户端发送的认证头；
- `replace_bearer`：删除已有认证头，从 Keychain 注入 bearer token；
- `replace_header`：从 Keychain 注入用户指定 header。

配置只保存 `secret_ref`，不保存明文 credential。App 设置页负责创建、替换和删除 Keychain secret；界面、日志、SQLite 和 Widget 不得回显。仍被 route 引用的 secret 不允许直接删除；删除或修改 route 后，最后一个引用消失的 secret 自动从 Keychain 清理。

认证注入属于显式 relay 模式。UI 必须明显区分 pass-through 与 relay，不能都描述为“完全透明”。

### 5.4 Pricing policy

协议 usage 可以统一，价格不能按协议推导。用户可以为 route/model 配置可选价格规则：

```json
{
  "id": "local-price-v1",
  "version": "1",
  "currency": "USD",
  "rules": [
    {
      "model_pattern": "example-*",
      "input_per_million": 1.0,
      "cached_input_per_million": 0.1,
      "output_per_million": 5.0
    }
  ]
}
```

计算结果一律为 `estimated_cost`，同时记录 policy ID 和 version。未配置价格时只展示 token，不显示零成本或猜测价格。

## 6. App 架构

```text
┌──────────────────────────── Transit.app ────────────────────────────┐
│                                                                     │
│  SwiftUI MenuBarExtra                                               │
│      │                                                              │
│      ├── RouteSettings ──→ ConfigurationStore ──→ Keychain          │
│      │                                      │                       │
│      └── Dashboard ←── UsageQueryService ←──┼── EventStore/SQLite   │
│                                             │                       │
│  ProxyService ─→ Router ─→ Upstream HTTP Client                     │
│      │                        │                                     │
│      └──── ProtocolObserver ←┘                                     │
│                    │                                                │
│              UsageEventQueue ───────────────→ EventStore            │
│                                                     │               │
│                                  WidgetSnapshotWriter               │
└─────────────────────────────────────────────────────┼───────────────┘
                                                      │ App Group JSON
                                             ┌────────▼────────┐
                                             │ Transit Widget  │
                                             └─────────────────┘
```

### 6.1 Xcode targets/modules

建议工程结构：

```text
Transit/
  App/                       SwiftUI App、菜单栏和设置
  Core/
    Domain/                  Route、UsageEvent、统计模型
    Proxy/                   NIO listener、router、upstream relay
    Protocols/               三个 usage adapter
    Storage/                 GRDB、migration、query
    Configuration/           JSON config、validation、Keychain refs
  Shared/                    App 与 Widget 共用的 snapshot DTO
TransitWidget/               WidgetKit 与 App Intents
```

`Core` 是源码模块边界，不是独立进程。UI 只能通过 application service 调用 Core，protocol parser 不依赖 SwiftUI、WidgetKit、GRDB 或 Keychain。

### 6.2 技术选型

- **UI**：SwiftUI `MenuBarExtra`；
- **本地 HTTP server**：SwiftNIO + NIOHTTP1；
- **上游客户端**：AsyncHTTPClient，复用 NIO event loop；
- **存储**：系统 SQLite + GRDB；
- **并发**：Swift Concurrency 负责 App 状态编排，NIO event loop 负责数据面；
- **桌面组件**：WidgetKit + App Intents；
- **配置**：Application Support 中的 versioned JSON；
- **Secret**：macOS Keychain；
- **登录启动**：`SMAppService`。

选择 Swift 单体 App 的原因：

- v1 只面向 macOS，不需要跨平台二进制；
- 无需安装和签名额外 helper；
- 不需要 Unix socket、Control API 和跨进程配置同步；
- UI、代理状态和 Widget 快照可以直接共享类型；
- 交付方式与普通菜单栏 App 一致，落地成本更低。

### 6.3 线程与隔离

- `ProxyService`、HTTP channel 和 body streaming 不运行在 `MainActor`；
- `ProxyService` 持有专用 `MultiThreadedEventLoopGroup`；
- `EventStore` 使用 GRDB writer queue，不在 NIO event loop 执行磁盘 I/O；
- NIO 只将完成的轻量 `UsageEvent` 非阻塞送入有界队列；
- `UsageViewModel` 位于 `MainActor`，只接收聚合结果和健康状态；
- App 终止时按 Proxy → event queue → database 顺序优雅关闭。

禁止在 SwiftUI view、`MainActor` 或 Widget extension 内处理代理流量。

## 7. App 生命周期

### 7.1 启动

1. 读取并迁移配置；
2. 打开数据库并执行 migration；
3. 校验所有 enabled route；
4. 启动对应 listener；
5. 发布代理健康状态；
6. 生成首份 Widget 快照。

单条 route 无效时不应导致整个 App 崩溃：该 route 标记为 invalid，其他有效 route 继续运行。端口冲突按 listener 隔离，面板显示准确错误和建议操作。

### 7.2 运行中修改

配置更新采用：

```text
validate → stage listeners → atomic persist → runtime commit → close old listeners
```

- 校验失败不改变运行状态；
- 新 listener 无法绑定时保留旧 listener；
- 持久化失败时关闭 staged listener 并保留旧配置；
- 仅 display name 或 pricing 变化时不重启 listener；
- upstream/protocol/auth 变化只替换相关 route；
- secret 更新后使相关 route 的新请求使用新值，不中断在途请求。

### 7.3 停止与退出

- “停止代理”只停止 listener，App 与统计界面继续运行；
- “退出 Transit”停止接收新请求，等待在途请求到超时上限，flush 事件后关闭数据库；
- 存在 enabled route 时退出需要提示 Agent 可能失去连接；
- 系统关机时尽力 flush，但不阻塞系统退出；
- App 崩溃后 SQLite WAL 保证已提交事件可恢复，未入队事件允许丢失。

### 7.4 登录启动

App 通过 `SMAppService` 注册登录启动。首次配置 route 后提示用户开启；是否开启由用户决定，不静默注册。

## 8. 数据面设计

### 8.1 请求转发

每个 listener 使用 NIO HTTP/1.1 server pipeline；上游连接由 AsyncHTTPClient 按 origin 复用。转发必须：

- 移除 `Connection` 及其声明的 hop-by-hop headers；
- 重建 upstream `Host`；
- 保留 `User-Agent`、认证头和业务 header，除非 route 明确替换认证；
- 不修改请求 JSON；
- 支持 `Content-Length` 与 chunked request body；
- 通过背压连接客户端读取与上游写入；
- 正确传播客户端取消、上游错误和超时；
- SSE chunk 到达后立即 flush，不等待完整响应。

上游连接超时为 30 秒，连续无数据读取超时为 10 分钟；请求不设置绝对总时长 deadline，因此持续发送 event 或 heartbeat 的长 SSE 不会在固定时刻被 Transit 截断。

为便于解析，v1 向上游请求 identity encoding。该变化只影响传输压缩，不改变响应业务内容；相关 framing/header 由代理正确重建。未来如保留压缩，应实现并行解压观察，不得完整缓冲。

### 8.2 增量观察

Observer 只观察流经的数据：

- 请求体使用有上限窗口提取顶层 `model`，超限后停止观察但继续转发；
- 普通 JSON 响应在配置上限内保留解析缓冲，超限标记 usage missing；
- SSE 使用增量行解析器，只保留当前 event 和累计 usage；
- 单 event 超限只关闭 observer，不关闭转发；
- 只把识别出的 usage 子对象交给 Event Builder；
- 客户端取消时尽力保留已收到的 usage，并把 outcome 标记为 `cancelled`。

Observer 的错误只能进入诊断状态，不允许向代理 channel 写入人为错误。

### 8.3 Protocol Adapter 接口

概念接口：

```swift
protocol UsageProtocolObserver: AnyObject {
    func observeRequest(contentType: String?, bytes: ByteBuffer)
    func observeResponse(contentType: String?, bytes: ByteBuffer)
    func finish(outcome: RequestOutcome) -> UsageObservation
}
```

每个 request 创建独立 Observer，并始终由对应 channel 的同一 NIO event loop 访问，因此不要求跨线程共享可变 parser 状态。

`UsageObservation` 包含归一化 usage、最小 raw usage、model 候选值和质量标记。Adapter 是纯协议解析器，不持有数据库、route、secret 或 UI 状态。

### 8.4 UsageEvent

```text
UsageEvent
  id
  started_at
  completed_at
  route_id
  agent_id
  protocol
  method
  endpoint_kind
  status_code
  outcome                  completed / failed / cancelled
  latency_ms
  model
  key_fingerprint
  input_tokens
  output_tokens
  cached_input_tokens
  reasoning_tokens
  total_tokens
  total_tokens_derived
  usage_quality            reported / missing
  usage_raw
  request_bytes
  response_bytes
  estimated_cost
  currency
  pricing_policy_id
  pricing_policy_version
  error_code
```

约束：

- `key_fingerprint` 使用 Keychain 中的本机随机 install secret 做 HMAC，避免跨安装关联；
- 不保存完整 URL query；
- `endpoint_kind` 由协议 Adapter 返回；
- `usage_raw` 只保存 usage 子对象并设置大小上限；
- `total_tokens` 优先保留上游值，字段相加推导时设置 `total_tokens_derived=true`；
- cost 只来自用户价格规则，永远是估算。

## 9. 存储与聚合

### 9.1 SQLite/GRDB

数据库位于 App container 的 Application Support。GRDB 负责：

- WAL；
- schema migration；
- writer queue；
- busy timeout；
- 聚合查询；
- retention 与 checkpoint。

核心表：

```sql
CREATE TABLE usage_events (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  completed_at TEXT NOT NULL,
  route_id TEXT NOT NULL,
  agent_id TEXT,
  protocol TEXT NOT NULL,
  method TEXT NOT NULL,
  endpoint_kind TEXT NOT NULL,
  status_code INTEGER,
  outcome TEXT NOT NULL,
  latency_ms INTEGER NOT NULL,
  model TEXT,
  key_fingerprint TEXT,
  input_tokens INTEGER,
  output_tokens INTEGER,
  cached_input_tokens INTEGER,
  reasoning_tokens INTEGER,
  total_tokens INTEGER,
  total_tokens_derived INTEGER NOT NULL,
  usage_quality TEXT NOT NULL,
  usage_raw TEXT,
  request_bytes INTEGER NOT NULL,
  response_bytes INTEGER NOT NULL,
  estimated_cost TEXT,
  currency TEXT,
  pricing_policy_id TEXT,
  pricing_policy_version TEXT,
  error_code TEXT
);
```

金额使用 decimal string 写入，避免浮点误差。索引至少覆盖 time、route+time、agent+model+time。

### 9.2 写入失败策略

- 数据面只进行非阻塞 enqueue；
- queue 满或 SQLite 不可写时，请求仍正常转发；
- App 维护 `events_dropped_total` 和最后存储错误；
- Dashboard/Widget 标记数据可能不完整；
- 不为保留统计而无限增加内存；
- event queue 容量由内置安全默认值控制，不接受无上限配置。

### 9.3 聚合查询

支持以下维度：

- 时间：最近一小时、今日、最近七天、自定义区间；
- `agent_id`；
- `route_id`；
- `protocol`；
- `model`；
- `usage_quality`；
- outcome。

结果包含 input/output/cache/reasoning/total tokens、请求数、失败数、延迟统计和可选 estimated cost。

## 10. 菜单栏 App 设计

### 10.1 菜单栏概要

- 代理健康状态；
- 今日 total token；
- 可选 estimated cost；
- 活跃请求数或最近错误标记。

### 10.2 面板

1. **Overview**：一小时/今日/七天的 input、output、cache、reasoning；
2. **Breakdown**：按 Agent、route、protocol、model 查看；
3. **Routes**：创建、编辑、启停 route，复制本地 base URL；
4. **Diagnostics**：端口冲突、上游错误、usage missing、丢事件数、SQLite 状态；
5. **Settings**：登录启动、数据保留、Widget 指标、Keychain secret。

界面不出现固定来源标签。名称、颜色和顺序来自用户配置；没有 usage 的 route 仍显示请求数、状态与延迟。

### 10.3 Route 编辑器

Route 表单包含：

- display name / agent label；
- 本地 port 与可选 path prefix；
- 用户输入的 upstream URL；
- protocol；
- authentication mode 与 secret reference；
- 可选 pricing policy；
- enabled 状态。

保存前只做本地 schema 和冲突校验，不自动访问 upstream。连接测试是独立、显式按钮，防止用户只想编辑配置时意外发起请求。

App 根据 route 生成本地 base URL 并允许复制，但不内置任何 Agent 的配置文件修改逻辑。

## 11. Widget 设计

Widget 只读取 App Group 中的 `widget_snapshot.json`：

```text
WidgetSnapshot
  generatedAt
  proxyState
  eventsComplete
  periods
    hour
      totalTokens / inputTokens / outputTokens
      cachedInputTokens / reasoningTokens
      estimatedCost / currency / topModels[]
    today
      ...
    seven_days
      ...
```

- 小尺寸：代理状态、总 token、input/output 摘要；
- 中尺寸：增加 cache、reasoning 和 Top Models；
- 用户可在 App 设置中选择总量、输入、输出、缓存或推理作为 Widget 主指标；偏好只保存在 App Group UserDefaults；
- App Intent 只切换 Widget 当前展示的一小时/今日/七天；三个周期的聚合值由 App 一次写入快照；
- cost 未配置时隐藏，不显示零成本；
- 数据不完整、App 未运行或快照过期时明确显示 partial/stale；
- Widget 不启停代理、不修改 route、不连接 SQLite、不读取 secret。

App 在事件聚合变化后做节流更新，无变化时通过低频 heartbeat 保持运行状态新鲜，并调用 `WidgetCenter.reloadAllTimelines()`。Widget timeline 定时刷新只用于更新时间和 stale 状态，不负责产生新 usage。

## 12. 配置设计

默认配置为空：

```json
{
  "version": 1,
  "routes": [],
  "pricing_policies": [],
  "storage": {
    "retention_days": 90
  }
}
```

配置文件位于 Application Support，由 App 原子写入。要求：

- 所有实体有稳定 ID；
- secret 只通过 Keychain reference 出现；
- URL 显式包含 scheme；
- listener 只允许 loopback；
- upstream 默认要求 HTTPS，HTTP 需要 route 级危险确认；
- port、prefix、route ID 和 pricing policy 在保存前统一校验；
- 未知字段报错，避免拼写错误静默忽略；
- 配置带 version，migration 先备份再写入；
- 运行时状态与持久化配置使用同一套 Swift model/validator。

## 13. 安全与隐私

- listener 只绑定 `127.0.0.1` / `::1`；
- upstream 默认仅允许 HTTPS；
- secret 存 Keychain，配置只保留 reference；
- HMAC install secret 与业务 credential 分开；
- 日志统一 header/body redaction；
- SQLite 不保存 prompt、completion、tool payload、完整 query 和明文 credential；
- App Group 快照只包含聚合数字和用户可见标签；
- relay 模式首次启用时明确提示 Transit 将代表客户端注入 credential；
- 主 App 优先启用 App Sandbox 的 outgoing/incoming network entitlement；Widget 必须沙箱化；
- 构建启用 Hardened Runtime，并验证签名产物可启动、主 App 与 Widget entitlement 正确；Developer ID 签名、notarization 与发布渠道验证属于公开分发工作，不作为应用实现完成的前置条件。

## 14. 诊断与错误状态

App 暴露以下正交状态：

- proxy：stopped / starting / running / degraded；
- route：disabled / invalid / binding / ready / upstream_error；
- request outcome：completed / failed / cancelled；
- usage quality：reported / missing；
- storage：healthy / degraded / unavailable；
- widget snapshot：fresh / partial / stale。

一次 HTTP 200 可以 `usage_quality=missing`；一次 cancelled 请求也可能已经收到完整 usage。UI 不把它们融合成含糊的单一“成功”状态。

诊断页提供：

- listener 和 route 状态；
- 活跃连接与请求数；
- protocol parse success/missing/error；
- events persisted/dropped；
- upstream latency 和状态分布；
- SQLite writer 状态；
- 最近一次配置应用结果；
- 显式 upstream 连接测试。

## 15. 测试策略与验收

### 15.1 Protocol fixtures

使用脱敏 fixture 覆盖三个协议：

- 非流式成功；
- SSE 分片跨 ByteBuffer；
- input/output usage 位于不同事件；
- cache/reasoning 明细；
- usage 缺失；
- 非法 JSON event；
- 单 event 超限；
- 客户端中途取消。

fixture 只使用虚构 host、model 和 token，不包含真实来源。

### 15.2 Proxy conformance

使用 NIO 测试客户端和本地 fixture upstream 的真实 socket 集成测试验证：

- method、path、query、业务 header 与正文语义一致；
- Content-Length 与 chunked request 均可用；
- SSE 首包和后续 chunk 不被完整缓冲；
- 背压有效且内存有界；
- 客户端取消会取消上游；
- observer/SQLite/Widget 更新失败不破坏转发；
- prefix 最长匹配遵守 segment 边界；
- pass-through 与 credential injection 不串 route；
- 日志和数据库不存在正文或明文 credential。

### 15.3 App 与 Widget

- route 增删改的原子切换；
- 端口冲突隔离；
- Keychain 创建、替换与删除；
- 数据库 migration、磁盘满和恢复；
- 登录启动注册与取消；
- App 退出时优雅关闭；
- App Group 快照和 Widget 注册；
- App 不运行时 Widget stale 展示；
- 使用本地 fixture 执行有界、确定性的 SSE heartbeat 回归，验证长连接在多个 heartbeat 期间保持开放、最终 usage 正确落库且代理仍为健康状态。

稳定性验证必须能在普通 `Transit` test scheme 中自动完成，不依赖真实 Agent、外部 endpoint、真实 credential、外部网络或长时间人工值守。长期 soak 可作为发布前附加观察，但不属于功能完成的硬门槛。

### 15.4 v1 验收条件

- 三个协议 fixture 全部通过；
- 通过用户配置的 loopback fixture route 完成端到端验证，覆盖路径拼接、认证策略、普通响应、SSE、上游错误和传输失败；验收不调用或假设存在任何真实 Agent、外部上游或 credential；
- 转发不存在完整响应缓冲；
- UI 操作不阻塞代理数据面；
- App 重启后配置与历史数据一致；
- usage 缺失时不生成伪造 token；
- source scan、持久化产物检查和 Release 二进制扫描确认无真实固定来源、固定 endpoint 与明文 secret；
- 本地签名的 sandbox Release 可启动，登录启动集成与 Widget 注册通过；Developer ID/notarization 在公开分发时另行验收。

## 16. Roadmap

| 阶段 | 内容 | 退出条件 |
| --- | --- | --- |
| v0.2 Core | Swift domain、route config、NIO proxy、三协议 observer | fixture 与 proxy conformance 通过 |
| v0.3 Data | GRDB migration、事件写入、聚合查询、价格规则 | 故障与恢复测试通过 |
| v0.4 Desktop | MenuBarExtra、route 设置、诊断、Keychain | 常驻和配置切换测试通过 |
| v0.5 Widget | App Group snapshot、WidgetKit、App Intents | 签名安装与 stale 场景通过 |
| v1.0 | 升级迁移、隐私审查、本地签名 Release 验证 | 确定性 fixture 验收与签名应用健康检查通过 |

Developer ID 签名、notarization、安装包和发布渠道验证属于 v1.0 实现完成后的分发工作，不改变 Transit 的运行时架构与功能验收结论。

## 17. 关键决策摘要

1. v1 是单体 Swift macOS App，不使用 Go daemon 和 agent-first CLI。
2. SwiftNIO/AsyncHTTPClient 在 App 内承担流式代理，GRDB 保存 usage 事件。
3. Transit 内置协议，不内置 Agent、上游、账户额度来源、真实 endpoint 和价格。
4. v1 只统计经过代理的请求级 usage，不合入来源特化的账户额度轮询。
5. 菜单栏窗口关闭不停止代理；退出 App 会停止代理，这是明确的产品约束。
6. Widget 只读 App Group 派生快照，不联网、不读数据库、不管理代理。
7. managed OAuth 不通过 MITM 强行覆盖；不能自定义 base URL 的流量明确不可观测。
