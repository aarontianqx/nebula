# Transit

Transit 是位于客户端与模型服务之间的 HTTP 代理，主要部署在 Linux，也可在 macOS 运行。普通路径流式透传，指定路径执行协议转换；调用 key 始终由客户端维护。

当前提供 provider 路由、默认 provider、模型别名、配置热加载，以及 Zed Edit Prediction 的 Completions → Chat 转换。运行日志按 UTC 日滚动，默认保留 14 天。不包含 token/费用统计或业务内容审计。

## 构建与启动

需要 Rust 1.88+，在 `transit/` 执行：

```bash
cargo build --release --locked
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/transit"
cp transit.example.toml "${XDG_CONFIG_HOME:-$HOME/.config}/transit/transit.toml"
```

编辑配置，替换示例 provider 的域名和模型 ID，然后：

```bash
./target/release/transit check
./target/release/transit serve
```

`check` 只校验本地配置，不访问上游、不创建日志。两条命令均支持 `--config /absolute/path/transit.toml`。`serve` 打印实际配置、监听地址和日志目录。

默认监听 `127.0.0.1:8787`，只允许本机访问。从另一台设备连接时，将 `server.listen` 配成服务器指定 IP，或用 `0.0.0.0:8787` 监听所有 IPv4 接口；客户端填写服务器实际 IP。监听地址修改需要重启。Transit 没有独立的调用方鉴权，应通过部署网络或入口控制访问，并在跨不可信网络时使用 TLS。

## 配置入口

完整字段、默认值见 [transit.example.toml](transit.example.toml)。模板默认使用 primary provider，`/secondary` 前缀选择 secondary provider，`/zed-edit/v1/completions` 选择 primary 并转换协议。provider 名称可自行定义。

`origin` 只包含协议、域名和可选端口。`base_path` 是额外加在请求路径前的前缀，通常不要重复放客户端已经携带的 `/v1`：

| 配置 | 客户端请求 | 上游地址 |
| --- | --- | --- |
| `origin = "https://api.example.com"`，`base_path = ""` | `/v1/chat/completions` | `https://api.example.com/v1/chat/completions` |
| `origin = "https://secondary.example.com"`，`base_path = "/gateway"`，剥离 `/secondary` 前缀 | `/secondary/v1/chat/completions` | `https://secondary.example.com/gateway/v1/chat/completions` |

规则的 `upstream_path` 覆盖完整上游 path，不再叠加 base_path。未命中规则时统一选择 default_provider。模型别名只在选定 provider 内替换，没有匹配的模型名保持原样。

provider 下还可定义具名 request_profiles，由路由通过 request_profile 显式选择，对最终上游 JSON 设置或删除参数。例如仅对 Edit 路由关闭 thinking、删除 temperature，不影响普通 Agent。参数需与目标模型匹配，详见 [请求配置](specs/features/routing.md#上游请求配置)。

更新 provider、规则、模型别名、请求 profile 或日志级别后，对服务进程执行 `kill -HUP <pid>`。校验失败保留旧配置，已有请求继续使用原配置。所有 server 参数和日志基础设施参数需要重启；不自动监听文件变化。

## Zed 接入

Agent 使用 OpenAI-compatible provider，API URL 填 `http://服务器地址:8787/v1`，key 使用对应上游的 key。

Edit Prediction 独立配置：

```json
{
  "show_edit_predictions": true,
  "edit_predictions": {
    "provider": "open_ai_compatible_api",
    "open_ai_compatible_api": {
      "api_url": "http://服务器地址:8787/zed-edit/v1/completions",
      "model": "edit-fast",
      "prompt_format": "qwen",
      "max_output_tokens": 512
    }
  }
}
```

在普通代码文件中输入后等待预测，或在命令面板执行 `editor: show edit prediction`，出现建议后按 Tab 接受。Edit Prediction 的认证仍在 Zed 侧配置。infer 依赖 Zed 对模型名的识别，自定义别名应显式指定格式；详见 [Zed 转换](specs/features/zed_edit.md)。

## 健康与日志

```bash
curl http://127.0.0.1:8787/_transit/health/live
curl http://127.0.0.1:8787/_transit/health/ready
```

健康检查不访问上游，不能证明 key、模型或 endpoint 可用。响应附带 X-Transit-Request-Id，可用于关联日志。Linux 默认日志在 `${XDG_STATE_HOME:-$HOME/.local/state}/transit/logs`，配置与日志分开存放。具体事件、保留与故障诊断见 [运维](specs/features/operations.md)。

## 部署与开发

- systemd 模板：[deploy/transit.service](deploy/transit.service)。需安装二进制、配置和专用用户；日志通过 StateDirectory 持久化。
- 容器：在本目录执行 `docker build -t transit .`；配置须监听容器可访问的地址，挂载整个配置目录及可写日志卷。细节见 [运维](specs/features/operations.md#部署)。
- 检查：`cargo fmt --all --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`。
- CI 执行以上检查、release 构建与模板校验；transit-v* tag 触发 Linux x64/arm64、macOS arm64 发布工作流。

## 维护文档

- [架构](specs/architecture.md)：技术选择、模块边界、生命周期和扩展方向。
- [路由与透传](specs/features/routing.md)：provider、模型映射、路径与 HTTP 边界。
- [Zed 转换](specs/features/zed_edit.md)：参数、prompt 格式、JSON/SSE 和已验证边界。
- [运维](specs/features/operations.md)：目录、日志、重载、健康、退出与部署。
