# Transit — v1 设计

> 状态：提案 (Proposal) · 起草于 2026-07-19
> 代码现状：`proxy.py`（PoC 已验证）位于项目根目录 `transit/`；本文件描述 v1 目标设计，尚未实现。
> 命名：取自天文"凌日"（transit）——流量如行星从恒星前穿过，代理从星光变化中推断看不见的信息。

## 1. 项目背景

日常使用多个 AI 编程 CLI（Kimi Code、Claude Code、Codex），每次模型请求的响应中都带有 token 用量（`input/prompt_tokens`、`output/completion_tokens`、`cached_tokens` 等，各家字段名不同），但没有一个统一的方式把这些数据收集起来做统计分析。

调研结论（2026-07，详见对话记录与官方文档）：

| 方案 | 覆盖范围 | 结论 |
|------|---------|------|
| 官方 OpenTelemetry 导出 | Claude Code ✅ / Codex ✅ / Kimi Code ❌ | 不通用 |
| 本地日志解析（ccusage 等） | Claude Code / Codex | 不支持 Kimi Code，且是事后统计 |
| LiteLLM / one-api / new-api 网关 | 通用 | 自带一层 key 体系（两层鉴权）；真正透明转发是 LiteLLM Enterprise 功能 |
| **透明反向代理（本项目）** | **通用** | **单层、零侵入，已验证可行** |

### 已验证的关键事实（PoC，2026-07-19）

1. 纯 stdlib Python 透明反代可以原样透传请求/响应（含 SSE 流式），并从中解析出 usage。
2. Kimi Code 的真实上游是 `https://api.kimi.com/coding/v1`（OpenAI 兼容协议）。实测响应 usage 样例：
   ```json
   {"prompt_tokens": 22377, "completion_tokens": 125, "total_tokens": 22502,
    "cached_tokens": 22272, "completion_tokens_details": {"reasoning_tokens": 74}}
   ```
3. **Kimi Code OAuth managed 登录有凭证保护**：无论改 `config.toml` 的 provider `base_url` 还是设 `KIMI_CODE_BASE_URL`，CLI 都会在本地判定登录失效（"OAuth login expired"），请求根本不会发出。→ OAuth 流量只能走 API key 模式或网络层拦截（mitmproxy 正向代理 + `NODE_EXTRA_CA_CERTS`，未实施）。
4. Kimi Code API key 模式（Console 创建，与会员额度共享）可自由指定 `base_url`，已实测全链路跑通。

## 2. 目标与非目标

**目标**

- 单进程、单端口的本地网关，同时承接多个 LLM provider 的流量，按路由转发到不同上游。
- 对客户端完全透明：不改写任何 header / body / 鉴权信息，代理只读不写。
- 每个请求落一条用量记录到本地 SQLite，维度：provider、API key（哈希）、客户端、模型、时间、延迟、token 明细。
- 可选 webhook 把记录实时外发到用户自建服务。
- 零第三方依赖（纯 Python 标准库），单文件可维护。

**非目标（v1 不做）**

- 多用户 / 配额 / 计费运营（那是 one-api 的领域）。
- 修改、重放、缓存请求。
- GUI；最多提供一个统计 CLI。
- OAuth managed 流量的网络层拦截（留作后续路线，见 §7）。

## 3. 系统设计

### 3.1 总体形态

```
kimi code ─┐
claude code ─┼─→ http://127.0.0.1:8787/<route>/... ─→ 路由匹配 ─→ 各 provider 上游
codex ───────┘        │
                      └─→ 响应流 tee 一份 → usage 解析 → SQLite / webhook / stdout
```

原则：**透传是底线，统计是旁听**。任何解析失败都不能影响转发主链路（fail-open）。

### 3.2 多 provider 路由：路径前缀方案

一个监听端口，用路径前缀区分出口。客户端只需把 `base_url` 指到 `http://127.0.0.1:8787/<route>`：

| 客户端 | 配置的 base_url | 出口上游 |
|--------|----------------|---------|
| Kimi Code | `http://127.0.0.1:8787/kimi` | `https://api.kimi.com/coding/v1` |
| Claude Code | `http://127.0.0.1:8787/anthropic` | `https://api.anthropic.com` |
| Codex | `http://127.0.0.1:8787/openai/v1` | `https://api.openai.com/v1` |

路由配置 `routes.json`（与可执行文件同目录，或 `~/.config/<app>/routes.json`）：

```json
{
  "routes": {
    "/kimi":      { "upstream": "https://api.kimi.com/coding/v1", "provider": "kimi" },
    "/anthropic": { "upstream": "https://api.anthropic.com",      "provider": "anthropic" },
    "/openai":    { "upstream": "https://api.openai.com",          "provider": "openai" }
  }
}
```

- 最长前缀匹配；命中后剥掉前缀，拼接上游路径（沿用 PoC 已验证的 join 逻辑）。
- 未匹配的请求返回 404 并在日志提示可用 route 列表——配置错误要一眼可见。
- **备选方案（不实施，留档）**：端口分流（一个进程监听多端口，每端口绑死一个上游）。仅当某客户端不允许 base_url 带路径时才需要；届时给 routes.json 加 `listen` 字段即可扩展。

### 3.3 usage 解析

按协议从响应体提取 usage，流式（SSE）与非流式都支持：

| 协议 | 输入 token | 输出 token | 缓存 token | 位置 |
|------|-----------|-----------|-----------|------|
| OpenAI Chat（Kimi） | `usage.prompt_tokens` | `usage.completion_tokens` | `usage.cached_tokens` 或 `prompt_tokens_details.cached_tokens` | 流式末段 chunk / 非流式 body |
| OpenAI Responses（Codex） | `response.usage.input_tokens` | `response.usage.output_tokens` | `response.usage.input_tokens_details.cached_tokens` | `response.completed` 事件 |
| Anthropic Messages（Claude） | `message_start.message.usage.input_tokens` | `message_delta.usage.output_tokens` | `usage.cache_read_input_tokens` | 流式多事件合并 |

解析失败返回 null，照常透传。原始 usage JSON 完整保留在记录的 `usage_raw` 列。

### 3.4 统计维度与存储

请求时刻打标，不靠事后推断：

| 维度 | 来源 | 说明 |
|------|------|------|
| `provider` | route 配置 | 路由时已知，最可靠 |
| `key_id` | `sha256(Authorization / x-api-key)` 前 12 位 | 绝不落明文；同 provider 多 key 自动分开；OAuth 场景 token 会轮换，此维度漂移，聚合时改用 provider+client |
| `client` | `User-Agent` | 区分同 provider 的不同 CLI |
| `model` | 请求体 JSON | — |
| 其他 | `ts` / `latency_ms` / `status` / `path` / `response_bytes` | — |

存储：SQLite（stdlib `sqlite3`），默认 `~/.transit/usage.db`：

```sql
CREATE TABLE usage_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts TEXT NOT NULL,               -- ISO8601 UTC
  provider TEXT NOT NULL,
  route TEXT NOT NULL,
  key_id TEXT,
  client TEXT,
  model TEXT,
  path TEXT,
  status INTEGER,
  latency_ms INTEGER,
  input_tokens INTEGER,           -- 归一化后
  output_tokens INTEGER,
  cached_tokens INTEGER,
  reasoning_tokens INTEGER,
  total_tokens INTEGER,
  usage_raw TEXT                  -- 原始 usage JSON
);
CREATE INDEX idx_usage_ts ON usage_events(ts);
CREATE INDEX idx_usage_dim ON usage_events(provider, key_id, model);
```

写入用单写线程 + 队列，避免多线程转发下的锁竞争。`USAGE_WEBHOOK_URL` 保留：每条记录落库后异步 POST 一份，失败仅告警不影响主链路。

### 3.5 客户端配置指引

- **Kimi Code**：`config.toml` 增加 `[providers.kimi-proxy]`（type=kimi，base_url 指代理，api_key 填 Console 创建的 key）+ 对应 `[models."proxy/<name>"]` 别名；用 `kimi -m proxy/k3` 或 `/model` 切换。已实测可行。默认 OAuth 模型不受影响。
- **Claude Code**：`ANTHROPIC_BASE_URL=http://127.0.0.1:8787/anthropic claude`。OAuth 订阅登录是否可过代理待实测（凭证保护风险同 Kimi）。
- **Codex**：`config.toml` 自定义 `model_provider` 的 `base_url` 指向 `/openai/v1`；Responses API 的 usage 透传需实测验证。

### 3.6 安全边界

- 只绑定 `127.0.0.1`，不对外暴露。
- 原样透传 `User-Agent` 等所有 header（Kimi 官方明确篡改 UA 会被封权益）。
- 明文 API key 只存在于客户端自己的 config 与上游之间；代理内存里短暂经过，不落盘，落库的只有哈希。
- 剥离 `Accept-Encoding` 强制 identity 以便解析（对客户端行为无实质影响）。

## 4. Roadmap

| 版本 | 内容 | 状态 |
|------|------|------|
| v0.1 PoC | 单上游透明反代 + usage 解析 + stdout/webhook 输出（`proxy.py`） | ✅ 已完成，Kimi 全链路实测通过 |
| v0.2 | routes.json 前缀路由、三协议归一化、SQLite 落库、维度打标、latency | 待实现 |
| v0.3 | `stats` 子命令/脚本：按天/provider/key/model 汇总；成本估算（内置价格表） | 待实现 |
| v1.0 | 整理为 nebula 正式子项目（定名、README、AGENTS.md）；评估 mitmproxy 正向代理模式承接 OAuth managed 流量 | 规划中 |

## 5. 遗留问题

1. Claude Code / Codex 的 OAuth 订阅流量是否像 Kimi 一样有凭证保护？需实测。
2. mitmproxy 正向代理模式（`HTTPS_PROXY` + `NODE_EXTRA_CA_CERTS`）作为 OAuth 流量的补充通道，验证成本与稳定性。
3. Codex Responses API 经代理后 usage 字段是否完整透传。
