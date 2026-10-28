# Provider、路由与透传

配置模板见 [transit.example.toml](../../transit.example.toml)，启动与 URL 示例见 [README](../../README.md)。

## Provider 与路径选择

Provider 是用户定义的上游，不是厂商枚举，包含 origin、base_path 和可选 model_aliases，不包含凭据。同一厂商的不同产品或部署可以定义为不同 provider。

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

adapter 路径始终应用所选 provider 的映射。其它 endpoint、multipart、query/path 内的模型名不改写。

配置了别名的 provider 在这些 endpoint 上要求未压缩的 JSON 对象，读取受 server.max_rewrite_bytes 和上传上限约束；不满足时返回 400/413/415，不静默跳过。顶层 model 如存在，必须是 1～256 字节字符串，字符限于 ASCII 字母、数字及 `-_.:/@+`。普通映射不补缺失 model，adapter 另行要求 model。

未命中别名时保留原 JSON 字节；命中时重序列化，只替换顶层 model，保留其它字段、顺序和数字精度。依赖原始 body 签名的请求不能透明改写，应为其配置不做映射的 provider。

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

除显式配置的改写外，不主动理解普通响应的业务字段。HTTP 200 的透传 SSE 内如包含业务错误，客户端可收到原事件，但 Transit 不解析它来分类最终结果；仅 adapter 理解其支持的流式 schema。

## 限额与错误

请求/连接/超时默认值统一见配置模板。连接数、在途请求数和上传大小分别受限；流式输出没有很小的总长度上限。背压依赖 HTTP body 消费，不创建无界响应队列。

- 本地错误返回 JSON error.type=transit_error，包含稳定分类 code 和受限 message。
- 非法参数 400，不支持 JSON 媒体/编码 415，上传或改写请求超限 413。
- 客户端上传空闲超时 408，上游连接失败通常 502，上游超时 504，draining/并发满 503。
- 响应头发出后截断或转换失败只能终止流，不能回写新 HTTP 状态或伪造成功 DONE。
- 每个代理响应附 X-Transit-Request-Id；保留上游自己的 x-request-id，不将 Transit ID 强制注入上游。

健康、重载和日志配置见 [运维](operations.md)。
