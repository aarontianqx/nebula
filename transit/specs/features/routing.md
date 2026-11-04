# Provider、路由与透传

配置模板见 [transit.example.toml](../../transit.example.toml)，启动与 URL 示例见 [README](../../README.md)。

## Provider 与路径选择

Provider 是用户定义的上游，不是厂商枚举，包含 origin、base_path、可选 model_aliases 和 request_profiles，不包含凭据。同一厂商的不同产品或部署可以定义为不同 provider。

- default_provider 必填且必须存在，任何未命中规则的路径都走它。
- 每条规则必须在 path（精确匹配）和 path_prefix 中选择一个，可选大写 method。
- 精确路径优先于前缀，前缀按完整路径段最长匹配。`/secondary` 匹配 `/secondary/v1/messages`，不匹配 `/secondaryx`。
- 同一 selector 下 method 重叠的规则在校验时拒绝，不靠文件顺序决定优先级。
- 命中后选择规则的 provider，省略时仍选默认 provider。不存在的 provider、重复 ID 或非法路径导致整份配置拒绝。
- 本地健康路径在代理规则之前处理。默认 provider 不是 fallback，401/404/429/5xx 不会切换其它 provider。

上游 URL 先按 strip_prefix 剥离匹配前缀，再加 provider 的 base_path。若指定 upstream_path，它覆盖整个上游 path，不再叠加 base_path。原始 query 字符串单独保留，不解码再编码。

origin 只接受 HTTP/HTTPS scheme 和 authority，不允许用户名、密码、query 或额外路径。配置路径必须是 ASCII origin-form，不含空白、query、fragment 或反斜杠。provider/规则名最长 128 字节，允许 ASCII 字母、数字、下划线和连字符。

## 模型别名

先按路径选 provider，再精确匹配该 provider 的 model_aliases。只替换一次，不做链式、通配符或跨 provider 搜索；未匹配则保留模型名。

例如 chat-latest 指向一个真实模型 ID，升级只修改这个映射并 reload。客户端别名不变，但新模型的参数、上下文和输出能力仍需确认。响应保留真实上游 model，不改写 JSON/SSE，也不合成 `/v1/models` 列表。

普通映射仅作用于 POST 请求，以及剥离入口前缀后的下列路径：

```text
/v1/chat/completions
/v1/completions
/v1/responses
/v1/messages
```

adapter 路径及通过 route.protocol 声明的自定义生成接口也应用所选 provider 的映射。其它 endpoint、multipart、query/path 内的模型名不改写。

配置了别名的 provider 在这些 endpoint 上要求未压缩的 JSON 对象，读取受 server.max_rewrite_bytes 和上传上限约束；不满足时返回 400/413/415，不静默跳过。顶层 model 如存在，必须是 1～256 字节字符串，字符限于 ASCII 字母、数字及 `-_.:/@+`。普通映射不补缺失 model，adapter 另行要求 model。

未命中别名时保留原 JSON 字节；命中时重序列化，只替换顶层 model，保留其它字段、顺序和数字精度。依赖原始 body 签名的请求不能透明改写，应为其配置不做映射的 provider。

## 上游请求配置

在 provider 下定义具名 request_profiles，路由通过 request_profile 选择。同名 profile 按最终选定的 provider 解析，省略 route.provider 时使用 default_provider 的定义。不选择 profile 的请求不应用任何额外参数策略，provider 也没有隐式全局 profile。

```toml
[providers.primary]
origin = "https://api.example.com"

[providers.primary.request_profiles.edit_prediction]
set = { thinking = { type = "disabled" } }
remove = ["temperature", "reasoning_effort"]

[[routes]]
id = "edit"
method = "POST"
path = "/edit/v1/chat/completions"
provider = "primary"
upstream_path = "/v1/chat/completions"
request_profile = "edit_prediction"
```

此示例参数需先确认上游/模型支持，Transit 不按厂商名猜测。更换 provider 或模型映射后应重新核对参数契约，配置检查只验证结构，不探测上游能力。

- 支持精确 POST 路由，既可配合 adapter，也可只改写 JSON 请求；不支持前缀或未指定 method 的广泛套用。
- 执行顺序为模型别名、可选协议转换、profile、发送上游。adapter 仍先验证客户端输入，不通过 profile 放宽未知字段规则。
- remove 删除顶层字段，缺失时无操作；set 覆盖顶层字段，对象整体替换，不递归合并。不支持 JSONPath、脚本或从环境读取值。
- 同一字段不能同时 set/remove；remove 不允许重复；profile 和字段名使用有界标识符。
- 禁止修改 model/messages/prompt/input/system/stream/n/echo 等结构字段，以及 headers/authorization/api_key 等认证字段。model 由映射管理，认证仍只透传。
- profile 必须存在于选定 provider；未引用的 profile 也会校验。未知字段、非法操作或引用错误使启动/reload 整体失败，不静默忽略。
- profile 路径需要未压缩 JSON 对象，即使没有模型别名也会有界解析。普通路径采用全局 rewrite/upload 上限，adapter 路径采用路由请求上限和全局上传上限。最终没有字段变化时，非 adapter 请求保留原 body 字节。
- profile 值支持 JSON 数据类型；TOML 无原生 null，需要恢复上游默认值时使用 remove，不能通过字符串 "null" 代替。
- profile 跟随配置快照热加载，只影响新请求；响应转换由 adapter 决定，profile 不改写响应。

## 认证与 HTTP 保真边界

客户端的 Authorization、x-api-key 等端到端认证头按原值转发，无任何 provider key、环境变量 key 或 key 池配置。请求不会因重定向或 fallback 被带去另一个 provider。

| 项目 | 行为 |
| --- | --- |
| body | 原始路径保留业务数据，包括未知字段、压缩内容和 SSE/NDJSON 事件，不完整缓冲响应 |
| headers | 保留多值结构，删除 hop-by-hop 及 Connection 指名的字段，重建 Host |
| forwarding headers | 删除客户端 Forwarded 和常见 X-Forwarded-*，不用于选择 provider |
| framing | HTTP 库处理 Content-Length/chunked，歧义 framing 拒绝，不保证 chunk/frame 边界或大小写一致 |
| trailers | 当前只转发 data，丢弃请求/响应 trailers 并删除 Trailer 声明，不承诺 trailer 签名或校验能力 |
| 压缩 | 原始路径不自动解压，需要 JSON 改写或 adapter 时有明确编码限制 |
| 3xx | 返回原始状态、Location 和 body，不跟随跳转 |
| 4xx/5xx | 原样返回上游状态、端到端 headers 与 body，另做受限日志诊断 |
| 取消 | 停止上传、释放上游响应；实际停止生成/计费取决于上游对断开的处理 |
| 传输范围 | 常见 HTTP 模型请求，不支持 CONNECT、Upgrade/WebSocket、特殊 1xx 语义 |

关闭 audit 时，除显式 adapter 外，不解析普通成功响应的业务字段。启用 audit 后，有界观察 JSON/SSE 的用量、模型、结束状态和业务错误，包括 HTTP 200 内的错误；仍向客户端转发原事件，观察失败不阻断透传。adapter 的协议转换校验仍独立执行。

## 限额与错误

请求/连接/超时默认值统一见配置模板。连接数、在途请求数和上传大小分别受限；流式输出没有很小的总长度上限。背压依赖 HTTP body 消费，不创建无界响应队列。

普通 JSON 改写默认 server.max_rewrite_bytes=33554432（32 MiB），全局 server.max_upload_bytes=67108864（64 MiB）。这是完整请求 body 的字节数上限，包含 JSON、工具定义和 Base64 图片等，不是 token 限额。adapter 使用路由 max_request_bytes（默认 2 MiB），不随全局改写限额增大；两类均受全局上传上限约束。JSON 改写会缓冲并解析 body，内存占用高于原始字节数，部署时应结合并发量调整限额。

- 本地错误返回 JSON error.type=transit_error，包含稳定分类 code 和受限 message。
- 非法参数 400，不支持 JSON 媒体/编码 415，上传或改写请求超限 413。
- 客户端上传空闲超时 408，上游连接失败通常 502，上游超时 504，draining/并发满 503。
- 响应头发出后截断或转换失败只能终止流，不能回写新 HTTP 状态或伪造成功 DONE。
- 每个代理响应附 X-Transit-Request-Id；保留上游自己的 x-request-id，不将 Transit ID 强制注入上游。

健康、重载和日志配置见 [运维](operations.md)。

## 协议声明与严格准入

协议按 method 和剥离 provider 入口前缀后的逻辑 path 识别，不按 provider 名称或认证头猜测。同一个 provider 可支持多个协议：

| method / 逻辑 path | protocol |
| --- | --- |
| POST /v1/chat/completions | openai_chat |
| POST /v1/completions | openai_completions |
| POST /v1/responses | openai_responses |
| POST /v1/messages | anthropic_messages |
| GET /v1/models | models |
| 其它 | raw |

base_path 只是上游路径前缀，不影响识别。adapter 自动声明客户端 completions、上游 chat。自定义 endpoint 可在精确 method/path 路由声明 protocol；upstream_protocol 默认相同，二者只决定准入/观察语义，不自动转换请求或响应。需要跨协议转换时仍须 adapter。

```toml
[[routes]]
id = "custom-chat"
method = "POST"
path = "/custom/chat"
provider = "primary"
upstream_path = "/vendor/chat/completions"
protocol = "openai_chat"
# upstream_protocol = "openai_chat"
```

protocol/upstream_protocol 不支持前缀或省略 method 的路由，避免一个声明放行整个路径空间。生成协议要求 POST，models 要求 GET；adapter 不能声明相冲突的协议。仅配置 provider 的前缀路由会继续识别剩余逻辑路径，例如 /secondary/v1/messages 合法，/secondary/.env 仍是 raw。

admission.strict=true 时，在上游连接前检查：

- 只允许上表中的接口或显式声明协议的精确路由；raw、CONNECT、Upgrade 和无效请求目标返回空 404。
- 默认要求非空 Bearer 认证；Anthropic Messages 也接受 x-api-key。不保存或校验上游 key，不添加独立服务 key。
- 生成接口要求未压缩 application/json 或 +json 对象，stream 如存在必须为布尔值。Chat 要求 model 和非空 messages（role、content 或工具调用结构）；Anthropic 另要求正整数 max_tokens；Completions 要求 model 和 prompt；Responses 接受 input 或 previous_response_id 的续接结构。保留未知扩展字段，不验证模型能力、工具 schema 或所有嵌套语义。
- GET /v1/models 要求空 body。JSON 读取受 max_rewrite_bytes/max_upload_bytes 约束，adapter 使用路由请求上限；超限、无效 JSON 或基本结构不符也返回空 404。
- 全局每秒最多接纳 requests_per_second 次准入尝试，采用固定一秒窗口，超限同样返回 404。连接/请求并发与空闲超时仍独立生效。
- 健康接口默认隐藏；expose_health=true 时允许 GET，响应无需认证，因此只应在受控网络暴露。

准入拒绝不附 Transit ID、不写逐请求审计记录；每 10 秒汇总拒绝数。HTTP parser 在应用之前拒绝的畸形请求可能表现为 HTTP 库错误或连接关闭。strict=false 时维持默认 provider 的宽松透传，认证头检查、协议 allowlist 和准入速率限制均关闭。

这些规则能过滤普通扫描，但不能阻止仿造合法 LLM JSON 的请求。公网私有服务的访问权限、TLS、IP 限流应在入口控制，不能把空 404 当作身份认证。
