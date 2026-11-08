# 运行与运维

启动、构建和配置入口见 [README](../../README.md)。本文维护目录、日志、健康、reload 和部署契约。

## 配置与日志目录

源码目录为仓库内独立的 transit/。运行数据不随 cwd 写入源码或 target。

| 环境 | 配置默认位置 | 日志默认位置 |
| --- | --- | --- |
| Linux 用户进程 | `$XDG_CONFIG_HOME/transit/transit.toml`，未设置时 `~/.config/transit/transit.toml` | `$XDG_STATE_HOME/transit/logs`，未设置时 `~/.local/state/transit/logs` |
| 用户级 systemd 模板 | 显式指定 `~/.config/transit/transit.toml` | XDG_STATE_HOME 为 `~/.local/state`，对应 `~/.local/state/transit/logs` |
| macOS 服务进程 | `~/.config/transit/transit.toml` | `~/Library/Logs/transit` |
| 系统级 systemd/容器模板 | 显式指定 `/etc/transit/transit.toml` | XDG_STATE_HOME=/var/lib，对应 `/var/lib/transit/logs` |

--config 可覆盖配置文件；logging.directory 可覆盖日志目录，必须是绝对路径，不进行 shell 的波浪号展开。Linux 的非空 XDG 变量必须是绝对路径，否则启动失败，不退回 cwd。

Unix 新建私有目录权限 0700，文件 0600，不改写已有目录/文件权限。每个日志目录使用 fs2 advisory lock 限制单进程写入，多副本使用独立目录。日志初始化、文件打开或初始写入失败会通过 stderr 报错并非零退出，不进入监听状态。

## 日志滚动与清理

- UTF-8 JSONL 文件为 transit.YYYY-MM-DD.jsonl，时间戳与文件日期使用 UTC；同日重启追加，跨日首次写入滚动。
- 每日滚动只限制时间跨度，不限制文件大小；不做压缩或按大小滚动，不应同时使用 copytruncate。
- 启动时和运行中每小时扫描过期文件，默认 retention_days=14。过期依据是已关闭文件的 mtime 早于当前时间减去 14×24 小时，删除发生在下一次扫描，文件数不严格等于 14。
- 只扫描第一层、严格匹配有效 transit.YYYY-MM-DD.jsonl 的普通文件，不跟随 symlink，不删活跃 writer 文件、锁、未知文件或配置。
- writer 与清理任务协调当前文件；未来时间戳或 metadata 错误保留并告警。单文件删除失败不中断其它清理，后台任务异常退出由主任务发现。

## 诊断事件

默认 Transit 模块 debug、依赖库 warn，文件与 stderr 分别过滤。stderr 默认 warn，最低可配 error；不直接使用可能输出敏感字段的 RUST_LOG 全局过滤器。

| 级别 | 主要事件 |
| --- | --- |
| INFO | startup/shutdown、reload_completed、成功请求最终结果、清理汇总 |
| DEBUG | 请求开始、provider/规则与映射、上游响应头、普通客户端取消、空闲连接关闭、日志心跳 |
| WARN | 上游 4xx、本地参数拒绝、无效 reload、容量拒绝、无法分类的连接失败、清理失败、reasoning-only 空补全 |
| ERROR | 上游 5xx、连接/超时、转换/截断、内部任务或日志基础设施错误 |

request_id 是 Transit 生成的 UUID，经 X-Transit-Request-Id 返回。日志按需包含 provider、rule_id、config_generation、method、adapter、请求/实际模型、状态、outcome、error_code、stage、耗时、首个上游 body 字节时间和上下行字节数。request_started 还记录选中的 request_profile 名称，不输出 profile 参数值。request_started/request_finished 均记录 request_path（客户端入口）和 upstream_path（应用前缀剥离、base_path 或路径覆盖后的上游路径），不含 query、scheme、host；不解码路径，每个字段最多保留 1024 个字符，超出追加 ...[truncated]。历史日志没有这些字段，不能补全过去的路径统计。

adapter 成功响应仅有 reasoning、没有补全文本时记录 WARN empty_completion，携带 request_id、reason=reasoning_only 和 streaming，不记录 reasoning 正文；该请求仍按 completed 记录，不计为转换失败。普通透传响应不解析此条件。

运行日志的 model 只在主链路原本已解析 JSON 时提取。启用 audit 后，普通透传请求的 model/stream 在后台提取入库，不回填先前运行日志。response_bytes 是已读取的上游 body 字节数，adapter 转换后客户端收到的大小可能不同。request_finished 的 completed 表示数据已交给 HTTP transport，不保证客户端应用读取或接受；后台观察出的业务失败另有 observation_result 日志，最终数据库 outcome 包含该结果。

运行日志会包含实际 provider 名、请求/解析后模型名、请求与上游路径（路径段也可能包含业务信息），以及启动时的配置路径、日志目录和监听地址，可能暴露部署信息或模型偏好。默认存放在仓库外的私有目录，不能作为公开示例直接提交或发布；分享排障日志前应进一步清理这些字段。示例配置、文档和测试仅使用通用 provider/model 值，不记录个人部署与联调配置。

HTTP 错误摘要使用最多 error_preview_bytes 的 body 前缀（默认 4096，0 关闭正文捕获），只提取已知 JSON error 的 type/code/param/message：

- 认证值、引号内片段、疑似 token、URL、代码/多行内容经过过滤；不记录完整 headers、query、prompt、正常响应或原始 SSE。
- 非 JSON、压缩、超限或截断无法解析时记录 unavailable 原因和受限上游 request ID，不 dump 原文。
- 有界预览不等待完整响应，已捕获摘要在定长响应完成、流失败或客户端取消时均尝试报告。
- 返回客户端时仍保留上游错误字节。启用 audit 后，已识别协议的成功 SSE 也会有界提取业务错误，HTTP 200 保持原样但最终 outcome 可记为 failed；adapter 内的 error 事件仍单独诊断。

字段过滤用于减少泄露风险，不等于允许公开日志，文件仍按私有运行数据管理。

## 日志故障可见性

tracing-appender 使用 8192 条写入的有界 lossy 队列和 16 KiB 单次事件写入上限。队列满丢日志而不阻塞模型流，通过 ErrorCounter 定期报告增量；writer 额外监测写入、flush 和日切换故障，向 stderr 告警并维护健康计数。文件恢复写入后报告 logging_recovered。

默认 WARN/ERROR 还输出 stderr，systemd/journald 或容器应采集它作为备用出口。日志故障不强杀模型请求；异步队列不承诺 SIGKILL、断电或文件/stderr 同时不可写时持久化。正常退出保留 worker guard 到最后，不逐行 fsync。

## 健康、reload 与退出

GET `/_transit/health/live` 返回 alive 和 logging_degraded、log_write_failures、log_events_dropped。GET `/_transit/health/ready` 返回 ready、config_generation、active_requests 和 logging_degraded。

健康不访问上游，也不验证 endpoint/key/model。draining 时 ready 返回 503；日志、审计、抓取和观察故障只改变诊断字段，不阻止模型请求。健康还提供 audit_enabled、audit_degraded、audit_write_failures、audit_dropped_records，observation_active、observation_skipped、observation_failures，以及 capture_enabled、capture_degraded、capture_failures，live 提供 admission_rejected。严格模式默认对健康接口返回空 404。计数是当前进程累计值，不代表缺失 token 的数量。

SIGHUP 重新读取完整配置，成功后原子切换，失败保留旧快照；已有请求使用旧 generation。支持修改默认 provider、provider/别名/request_profiles、路由/协议声明、admission、logging.level/stderr_level/error_preview_bytes，以及除 directory 外的 capture 参数。切换 provider 时如缺少路由引用的 profile，校验失败并保留原配置。

所有 server/dashboard/audit 参数、capture.directory，以及 logging.directory/rotation/retention_days/cleanup_interval_seconds 变化需要重启。如果 reload 文件包含此类修改，整份 reload 拒绝，不部分应用。没有自动文件监视。建议原子替换配置文件后再 SIGHUP；用户级 systemd 使用 `systemctl --user reload transit`，系统级使用 `sudo systemctl reload transit`。reload 只发送信号，命令成功不代表应用已接受配置，需在应用日志中确认 reload_completed 或拒绝原因。

SIGTERM/SIGINT 开始 drain，默认 grace=180 秒，停止接收新连接/请求，尽量完成在途请求，超时后强制结束。重启仍会造成短暂的新连接不可用；SIGHUP 不关闭监听及已有流。部署 stop timeout 应大于 grace 并留出清理余量。

## Web 统计管理端口

`dashboard.enabled=true` 启动内置网页和只读 API，要求同时启用审计。默认关闭，监听 `127.0.0.1:8788`，必须使用与代理不同的非零端口；dashboard 的开关与监听地址需要重启。代理端口不提供看板路由，严格准入和上游凭据透传行为不变。

网页、CSS、JavaScript 编译进可执行文件，不依赖 Node.js 或外部资源。默认近 24 小时，提供近 1 小时/7 天/30 天和自定义范围；时间显示采用浏览器时区。支持 provider、请求/解析后/响应模型、path、outcome 的精确筛选，以及每页 50 条的请求明细。模型分布按响应模型、解析后模型、请求模型依次回退，显示请求最多的 5 个组合，标签为 `provider/model`；点击同时筛选 provider 和模型。

调用趋势使用统一 SVG 坐标系绘制柱子、纵轴数值与网格，横轴显示中间时间刻度并随宽度调整数量。悬浮绘图区（含空白）显示横向数值参考线、纵轴读数及时间段的请求数或 Token；时间精确到分钟，跨日长范围附日期。Token 卡片统一使用浅色并标为“Token 总量”，统计口径在折叠诊断中说明。

连接状态和更新时间合并在顶部 header，浏览器时区说明放在更新时间提示中。请求表格使用时间、provider/model、path、结果、耗时、token 六列，每条记录单行显示。Token 按“输入 / 输出 / 总计”合并展示，缺失值显示“—”，用量完整状态保留在悬停提示及请求详情中。正常完成显示 HTTP 状态码；HTTP 失败显示状态码，HTTP 200 等状态内的业务失败显示“失败 · 200”；取消、中断、未完整结束和待结束各自显示状态。悬停结果可查看 HTTP 状态和错误代码，点击整行查看完整记录，行悬停加深，支持 Enter/空格键打开；弹窗标题固定，正文独立滚动，滚动条仅出现在内容区。

管理 API：

- `GET /api/stats`：汇总、趋势、前 12 个模型/provider 组合、筛选选项和当前页请求。
- 参数 `from_ms`/`to_ms` 为 UTC 毫秒，范围 `[from_ms,to_ms)`，默认近 24 小时，单次最多 366 天；其它参数为 `provider`/`model`/`path`/`outcome`，均使用绑定参数。
- `limit` 默认 50、最大 100。后续页使用 `next_cursor` 中的 `before_ms`/`before_id`，按 started_at_ms/request_id 降序查询；分页时沿用相同时间窗口，避免新请求导致偏移分页重复。
- `GET /api/requests/<request_id>`：单条完整审计记录，不提供原始抓取或配置内容。

Token 汇总、趋势与模型分布只累加完整用量，缺失不作零估计；缓存与 reasoning 是输入/输出的子集。输入缓存命中率独立展示，为缓存字段有效的完整记录中，缓存读取 token 总数 / 输入 token 总数，按 token 加权，不是命中过缓存的请求占比。缓存字段为 NULL、负数或超过该条输入量时不参与分子或分母；显式 0 是有效未命中，输入总数为 0 时显示未知。主卡片显示命中率和缓存 token 数，有效样本条数留在折叠诊断中，避免把缺失字段当作 0。请求概览并排显示总请求和已完成数，字号相同；完成率为 completed / 已结束记录，作为辅助信息，取消、异常及进行中分别计数。失败、未完整结束、取消和中断计入未完成数。in_progress 单列，可能表示请求进行中，也可能是后台尚未完成更新。用量完整情况及缓存字段覆盖情况放在默认折叠的“统计诊断”中，不占主卡片。仅正常完成（completed）的已识别生成请求，用量部分或缺失时在 Token 卡片显示提示；取消、中断、失败、进行中及非生成接口（如 models/raw/countTokens）没有用量不触发此提示。用量不完整表示缺少统计依据，也可能是上游未提供 usage，不自动判定为代理故障。每次看板查询使用同一 SQLite 读事务快照；记录异步写入，刷新后仍可能发生后续修正。

SQLite 使用单独的只读连接和线程，`query_only=ON`，不访问上游。队列最多等待 8 个任务，单任务从入队开始最多 3 秒，SQLite progress handler 中断超时查询，锁等待最多 100ms；HTTP 等待最多 4 秒，满队列或失败返回 503。查询使用现有时间索引、有限返回量和 2 MiB SQLite page cache；大量历史查询仍会消耗共享磁盘/CPU，管理端口限制 64 个连接，不能作为无限并发分析接口。

管理响应禁止浏览器缓存和跨站 iframe，使用仅同源静态资源的 CSP；不开放 CORS，拒绝 `Sec-Fetch-Site: cross-site`，Host 只接受 IP 地址或 localhost，以减少浏览器跨站读取和 DNS rebinding 风险。反向代理使用域名入口时，应保留同源路径并将转发 Host 改为管理监听 IP。网页用文本节点展示数据库字段，不将模型名或错误摘要当作 HTML。

管理端口不提供身份认证，记录包含私有模型/provider/path 和错误信息。默认绑定本机；远程使用受控内网/VPN、SSH 隧道或有认证的反向代理，不直接发布公网。退出时与代理一起停止接纳新连接，再结束查询线程；管理故障记录 dashboard_query_failed，监听失败或管理 server 异常退出会使服务显式失败。

## 请求响应抓取

`[capture] enabled=true` 后，已接纳且匹配 providers 的请求生成独立 JSONL；不依赖 audit 开关，准入拒绝和健康请求不生成文件。providers 默认为空数组（所有 provider），填写时必须引用已配置的 provider。request_body 默认 false，response_body 默认 true，可单独设置。默认目录 `$XDG_STATE_HOME/transit/captures`，未设置 XDG 时为 `~/.local/state/transit/captures`，macOS 同此状态目录；directory 可覆盖绝对路径。

文件路径为 `YYYY-MM-DD/<request_id>.jsonl`，日期使用 UTC，每行有 type、timestamp_ms：

| type | 内容 |
| --- | --- |
| request_started | schema_version=1、起始请求元数据、白名单请求头、body 来源与限额 |
| response_headers | 上游 HTTP 状态和白名单响应头；未取得上游响应时可能没有 |
| body_chunk | direction=request/response、offset、bytes、data_base64 |
| request_finished | 转发结束元数据 summary、完整性、各方向 seen/queued/stored 字节及截断/存储错误；不等待异步观察结果 |

请求 body 是客户端原字节，位于模型改写/协议转换之前；响应 body 是上游原字节，位于 adapter 之前。片段不对应 SSE 事件，也不保证与网络帧边界一致。压缩与二进制原样保存，不执行解压或业务字段解析。可按文件行顺序筛选 direction=response 的 body_chunk 并拼接 Base64 解码结果；检查 response_headers 的 content-encoding 后再决定是否解压。例如在本地重建响应：

```python
import base64
import json
from pathlib import Path

with Path("request.jsonl").open() as source, Path("response.bin").open("wb") as target:
    for line in source:
        event = json.loads(line)
        if event["type"] == "body_chunk" and event["direction"] == "response":
            target.write(base64.b64decode(event["data_base64"]))
```

头字段只允许 content-type、content-encoding、content-length、accept、accept-encoding、anthropic-version、anthropic-beta、x-request-id、request-id，值有长度限制并做通用脱敏；认证头、cookie 和任意扩展头不保存。原始 body 不脱敏，可能包含代码、对话、工具数据或正文中出现的凭据；即使只抓响应也应视为私有内容。Unix 新建目录 0700、文件 0600，已有权限不改写。日志和 SQLite 不因此存入完整 body；不要分享或提交抓取文件。

max_body_bytes 默认每方向 8 MiB，可配置 1 字节到 64 MiB；截断只影响抓取，不改变上游请求或客户端响应。专用线程使用 512 槽有界队列，body 每片最多 16 KiB，队列满停止该次抓取并预留结束记录。capture_complete=true 表示启用的方向没有已知截断/队列/存储/请求处理错误，不代表客户端应用已消费全部响应。HTTP 错误或业务错误的完整响应也可能具有完整抓取。客户端取消、处理错误、大小超限等会标记 false；强杀可能没有结束行。seen_bytes 是启用方向已观察到的字节，queued_bytes 是排入队列的字节，stored_*_body_bytes 是实际写入文件的字节。

max_total_bytes 默认 1 GiB，最小 64 KiB，按受管理 JSONL 的实际编码后大小计算，含 Base64 开销。启动、约每分钟及空间不足时清理：超过 retention_days（默认 3 天，可配置 1–36500 天）的已结束文件按 mtime 删除，再从最旧文件开始腾出预算。只处理有效日期目录中的 UUID.jsonl 普通文件，不跟随 symlink，不删除当前活跃文件及未知文件。为每个活跃文件预留 16 KiB 结束元数据；空间不足时停止抓取，保留可写的结束记录。多实例须使用独立目录，不能共享此存储预算。

启用、provider 筛选和 body 开关/上限热加载只作用于新请求；已有流继续按原设置抓取。目录级预算和保留期采用新设置，降低预算时已有活跃文件及结束预留可能暂时超过新值，完成后再清理。关闭抓取不主动删除已有文件；本次进程曾启用抓取时仍维护清理，启动即关闭则不访问抓取存储。

抓取采用尽力写入，不等待磁盘/队列，不逐行 fsync。正常关闭先 drain，再等待 writer 写完；强杀/断电不保证完整。故障通过 capture_failed、capture_incomplete 日志及健康计数可见；完整写入记录 capture_completed（DEBUG）。存储错误不修改代理响应或阻止新请求；需要可查询的元数据统计时单独启用 audit，审计同样采用尽力记录语义。

## SQLite 请求审计

[audit] enabled=true 后，已接纳的代理请求写入 requests 表；严格准入拒绝、健康检查和接纳前的容量拒绝不写入，容量拒绝仍有运行日志。旧配置默认关闭，不从历史日志回填用量。

默认数据库为 `$XDG_STATE_HOME/transit/transit.sqlite3`，未设置时为 `~/.local/state/transit/transit.sqlite3`。macOS 的数据库使用 `~/.local/state/transit`，日志仍在 Library/Logs。audit.database 可覆盖绝对路径。数据库、-wal、-shm、.sqlite3.lock 为私有运行数据；Unix 新建数据库/锁 0600，目录 0700，同一路径只允许一个 Transit writer。系统级 systemd 模板为 `/var/lib/transit/transit.sqlite3`，用户级模板沿用用户状态目录。不要提交数据库，也不要把包含实际模型/provider/path 的查询结果直接公开。

每条记录包括：

| 类别 | 字段 |
| --- | --- |
| 关联与时间 | request_id、UTC 毫秒 started_at_ms/finished_at_ms、config_generation、rule_id |
| 路由 | method、request_path/upstream_path（不含 query/host，最多 1024 字符）、provider、client_protocol/upstream_protocol |
| 模型 | requested_model、别名后的 resolved_model、上游 response_model；无法提取时为空 |
| 传输 | streaming、http_status/upstream_status、duration_ms、response_headers_ms、first_body_byte_ms、request_bytes/response_bytes |
| 结果 | outcome、error_source/stage/code/message、finish_reason、response_status |
| 用量 | input_tokens、output_tokens、total_tokens、cache_read_tokens、cache_write_tokens、reasoning_tokens、usage_json、usage_status |
| 观察质量 | observer_status、observer_error |

不保存认证头、query、prompt/messages、工具参数、正常文本或 reasoning 内容。usage_json 仅保存允许列表中的数值字段及数值明细，未知扩展内容不会原样存储。错误摘要有界脱敏，但厂商错误仍可能包含业务信息，数据库应按私有数据管理。

协议观察支持 OpenAI Chat/Completions/Responses、Anthropic Messages 与 Gemini GenerateContent 的 JSON/SSE，只看 model、usage、结束标志和 error。不重写响应、不注入 stream_options；OpenAI 流若客户端未请求 include_usage，通常拿不到 token。Anthropic message_start/message_delta 的累计用量覆盖合并，不将每次 output_tokens 累加。统一 input_tokens 包含 Anthropic 普通输入、缓存读取和缓存写入；OpenAI 的 cached/reasoning 已包含在 input/output 内，不再加到 total。缺失值为 SQL NULL，不能当成 0。

用量按已识别的上游协议解析，再归一化到数据库和看板 API；不根据 provider 名称猜测字段。下表中的字段位于 `usage`，Gemini 位于 `usageMetadata`：

| 协议 | 输入 / 输出 | cache_read_tokens | cache_write_tokens |
| --- | --- | --- | --- |
| OpenAI Chat / Completions | prompt_tokens / completion_tokens | prompt_tokens_details.cached_tokens | prompt_tokens_details.cache_write_tokens |
| OpenAI Responses | input_tokens / output_tokens | input_tokens_details.cached_tokens | input_tokens_details.cache_write_tokens |
| Anthropic Messages | input_tokens + cache_read_input_tokens + cache_creation_input_tokens / output_tokens | cache_read_input_tokens | cache_creation_input_tokens |
| Gemini GenerateContent | promptTokenCount + toolUsePromptTokenCount / candidatesTokenCount + thoughtsTokenCount | cachedContentTokenCount | 未提供，保留 NULL |

缓存读取和写入是输入的细分计数，不再加到 total_tokens；Anthropic 仅在归一化输入时相加。usage_json 保留允许列表中的原始字段名，不随数据库列重命名。数据库 schema v2 在启动时通过事务将 v1 的 cached_input_tokens/cache_write_input_tokens 重命名为 cache_read_tokens/cache_write_tokens，保留历史值和 NULL；旧版程序不支持 v2 数据库，不能仅替换旧二进制来回退。

usage_status 为 complete / partial / missing：complete 需要输入输出均已知、观察到协议终结且传输正常完成；取消、截断或观察失败时保留已收到的计数并标 partial，完全没有用量为 missing。complete 仅表示已观察到完整计数，不证明模型计费准确。usage_json 保留原口径，便于核对归一化结果。

observer_status 为 pending / observing / observed / partial / unsupported / parse_error / dropped。观察只适用于已识别的生成协议，使用最多 8 KiB 内容前缀判断 JSON/SSE，兼容缺失或错误的 Content-Type；不根据请求 stream=true 强行判定格式，因此 JSON 错误仍可识别。未知 body 不强行解析，原因写入 observer_error。该容错不改变准入规则或 adapter 的转换校验。

独立观察运行时固定两个线程，避免解压/解析占用转发运行时。Content-Encoding 支持 identity、gzip（含拼接成员）、deflate（HTTP 标准的 zlib 包装）、br、zstd，最多三层按逆序解码；zstd 窗口最多 8 MiB。不改写 Accept-Encoding、响应头、原始转发或 capture 字节。损坏/不完整的编码记录 response_decode_failed；未支持编码记录 unsupported_content_encoding。

audit.max_request_json_bytes、max_response_json_bytes 默认各 8 MiB，max_sse_event_bytes 默认 1 MiB；后两者按解压后字节计算。max_decoded_response_bytes 默认 64 MiB，限制每个响应的累计观察字节（含 SSE），超限只停止统计。后台每次处理最多 16 KiB 后让出执行权。max_pending_observations 默认 256；每请求每方向最多排队 64 个片段，全局 observer_queue_bytes 默认 32 MiB，计入排队片段和保留的解析输入，不含队列对象、解压窗口、容器容量及 JSON 对象的额外开销。不要将该值视为整个进程 RSS 上限。

前台只尝试获取资源和入队，不等待队列或后台解析。观察容量满时新请求记录 dropped/observer_capacity，资源或队列不足时停止该次响应观察，已提取用量保留为 partial；没有用量为 missing。后台请求 model/stream 采样也受大小和队列限制，缺失时保留 NULL，并记录 request_observation_skipped。失败/跳过分别有 observation_failed、observation_skipped；成功有 observation_completed。已解析的业务失败或 incomplete 另记录 observation_result，统计与运行日志通过 request_id 关联。

outcome 包括 in_progress、completed、failed、incomplete、cancelled、interrupted。HTTP 200 内的业务错误可记为 failed；Responses incomplete/cancelled 记为 incomplete；到达 max_tokens 的正常 finish_reason 仍是 completed。error_source 区分 local、client、transport、upstream_http、upstream_body。客户端取消时不把已知 token 清零，也不能保证上游已停止计费。

审计不会单独要求前台缓冲完整上传；普通透传的 model/stream 由后台读取原始请求副本提取，压缩请求的请求模型可能为空。字段改写、adapter 或严格准入所需的读取和解析仍属于主链路。响应侧始终消费驱动，不等待解压、完整 SSE、统计或数据库提交才转发。

SQLite 使用 WAL、synchronous=FULL 和专用线程。起始行只做非阻塞入队，结束更新使用预留队列槽；模型请求不等待起始行或最终更新落盘。队列不可用时跳过该请求的审计，记录 audit_record_dropped/request_audit_skipped 并增加 audit_dropped_records；数据库锁或写入失败记录 audit_write_failed 和 audit_write_failures，后续仍尝试写入，成功后报告 audit_recovered。运行时故障不返回 503，也不降低 ready；启动时数据库初始化、版本和目录锁校验仍须成功。

记录可能在请求结束后稍晚出现或更新，in_progress 可能表示后台仍未处理完，不能精确等同于当前在途连接数。结束更新失败的行可能暂留 in_progress，下次启动统一标 interrupted；强杀、进程崩溃或过载可能丢失整条记录或最后用量。这是最终一致的尽力统计，不承诺无损审计。正常退出先 drain 请求，再等待观察运行时最多额外 10 秒，随后刷新 SQLite 队列并 checkpoint；持续磁盘阻塞可能延长存储清理阶段，部署退出超时需留余量。

默认保留 90 天，启动及每小时按 finished_at_ms 清理已结束记录，空闲时也执行；不删除 in_progress。删除会复用空页，不自动 VACUUM 缩小文件。正常退出 drain 后刷新队列并 checkpoint。在线备份请用 SQLite backup API 或 sqlite3 `.backup`，不要只拷贝活跃 .sqlite3 而漏掉 WAL；数据库应放本地持久磁盘，不使用网络文件系统共享 WAL。

示例统计（sqlite3 CLI 可单独安装，服务运行不依赖它）：

```sql
-- 最近请求；毫秒时间戳转换为 UTC。
SELECT datetime(started_at_ms / 1000, 'unixepoch') AS time_utc,
       request_path, provider, resolved_model, outcome, duration_ms,
       input_tokens, output_tokens, usage_status, error_code
FROM requests ORDER BY started_at_ms DESC LIMIT 30;

-- 只对完整用量求和；仍应单独查看缺失/部分记录数。
SELECT provider, resolved_model, count(*) AS requests,
       sum(CASE WHEN usage_status = 'complete' THEN total_tokens END) AS known_tokens,
       sum(usage_status = 'partial') AS partial,
       sum(usage_status = 'missing') AS missing
FROM requests WHERE outcome != 'in_progress'
GROUP BY provider, resolved_model;
```

用量口径参考：[OpenAI Chat stream_options](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[OpenAI Responses 流事件](https://developers.openai.com/api/docs/guides/streaming-responses)、[Anthropic 流式消息](https://platform.claude.com/docs/en/build-with-claude/streaming)、[Anthropic 缓存用量](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)、[SQLite WAL](https://sqlite.org/wal.html)。新增协议版本时应核对字段与终结事件，而非把不同协议的计数直接相加。

## 部署

### Linux 用户级 systemd（推荐单机部署）

仓库提供通用模板和操作流程；实际配置、程序及运行数据安装在宿主机，不提交个人用户名、地址、provider 或模型配置。无需修改应用代码。服务由用户级 systemd 管理，不依赖终端或开发会话存活。

| 用途 | 安装位置 |
| --- | --- |
| 可执行文件 | `~/.local/bin/transit` |
| 应用配置 | `~/.config/transit/transit.toml` |
| systemd unit | `~/.config/systemd/user/transit.service` |
| 日志、审计数据库和抓取 | `~/.local/state/transit/` 下的原有目录和文件 |

模板 [transit.user.service](../../deploy/transit.user.service) 中 `%h` 表示运行用户的 home，显式设置 XDG_CONFIG_HOME 和 XDG_STATE_HOME 为上表目录。自定义 XDG 目录时同步调整本机 unit 和以下命令；logging.directory、audit.database、capture.directory 的绝对路径覆盖仍有效。unit 不依赖源码目录或 shell PATH，不加载 shell 配置。

首次安装，在 `transit/` 下执行构建和目录准备；已有匹配宿主机平台的发布二进制时，可跳过构建并替换二进制来源：

```bash
cargo build --release --locked
install -d -m 755 "$HOME/.local/bin"
install -d -m 700 "$HOME/.config/transit" "$HOME/.config/systemd/user"
if [ ! -e "$HOME/.config/transit/transit.toml" ]; then
  install -m 600 transit.example.toml "$HOME/.config/transit/transit.toml"
fi
```

编辑配置，替换示例上游及模型，按访问范围设置监听地址。保留已有配置、数据库和日志；不要在更新时用示例覆盖实际配置。随后校验并安装二进制：

```bash
install -m 755 target/release/transit "$HOME/.local/bin/transit.new" &&
  "$HOME/.local/bin/transit.new" check --config "$HOME/.config/transit/transit.toml" &&
  mv -f "$HOME/.local/bin/transit.new" "$HOME/.local/bin/transit"
```

如果已有手工进程或临时服务，先按下文迁移，避免两个实例争用端口和存储。安装持久 unit 并启用自启动：

```bash
install -m 644 deploy/transit.user.service "$HOME/.config/systemd/user/transit.service"
sudo loginctl enable-linger "$(id -un)"
systemctl --user daemon-reload &&
  systemctl --user enable --now transit.service
systemctl --user status transit.service --no-pager
loginctl show-user "$(id -un)" -p Linger
```

`enable` 将服务加入用户管理器启动目标；`linger` 使用户管理器在开机时启动，且退出登录后继续运行。预期 Linger=yes；这一步可能需要管理员授权。模板默认异常退出 5 秒后重启，持续快速失败仍受 systemd 启动频率限制；修复配置后可执行 `systemctl --user reset-failed transit` 再启动。正常停止不会自动重启。

日常操作：

```bash
systemctl --user status transit --no-pager
systemctl --user reload transit
systemctl --user restart transit
systemctl --user stop transit
journalctl --user -u transit -n 50 --no-pager
journalctl --user -u transit -f
```

journal 保存启动、退出和 stderr，应用 JSONL 日志继续按原有规则滚动清理。检查 unit 为 active/running、配置的监听端口可达及应用日志；健康接口仅在准入允许时可用，不验证上游 key 或模型。

更新程序时重新构建，或准备可信的匹配平台发布产物，在 `transit/` 执行：

```bash
cargo build --release --locked &&
  install -m 755 target/release/transit "$HOME/.local/bin/transit.new" &&
  "$HOME/.local/bin/transit.new" check --config "$HOME/.config/transit/transit.toml" &&
  mv -f "$HOME/.local/bin/transit.new" "$HOME/.local/bin/transit" &&
  systemctl --user restart transit
systemctl --user status transit --no-pager
```

始终先写同一目录中的新文件并校验，再用 rename 原子替换部署文件；不要直接 cp/install 覆盖正在执行的目标，否则可能报 Text file busy。替换后旧进程继续使用旧 inode，直到 restart 才运行新版。保留可信的旧版本产物可按相同流程回退；若未来有数据库不兼容迁移，需先确认其兼容性，替换二进制不等于回退运行数据。更新 unit 后还需 daemon-reload；程序替换不需要 daemon-reload，reload 不加载新版程序。

restart 使用 SIGTERM 等待应用 drain；模板 TimeoutStopSec=210 秒，覆盖默认 shutdown_grace_seconds=180 秒并留出收尾时间。调整应用 grace 时同步提高停止等待；超出 grace 的请求仍可能被中断，重启期间新连接短暂不可用。

迁移已有进程：

- 手工启动的进程：确认 PID 和配置后发送 SIGTERM，等待退出和端口释放，再启用持久服务。不要同时启动第二个 writer。
- `systemd-run --user --unit=transit` 创建的同名临时服务：先用 `systemctl --user show transit.service -p Transient -p ActiveState` 确认，再执行 `systemctl --user stop transit.service`。等待该单元卸载（LoadState=not-found）后再安装并加载持久 unit；临时单元未卸载时不要启动同名持久服务。失败状态的临时单元可先 reset-failed 再检查。切换后用 `systemctl --user show transit.service -p FragmentPath -p Transient -p MainPID` 确认路径指向用户配置文件、Transient=no，并检查 `systemctl --user is-enabled transit.service` 返回 enabled。
- 用户级与系统级部署二选一；二者即使同名也由不同管理器控制，不能重复启动同一配置和状态目录。

### 系统级 systemd

需要独立服务账户或由管理员统一运维时，使用 [transit.service](../../deploy/transit.service)：

1. 创建专用 transit 用户/组，安装二进制到 /usr/local/bin/transit。
2. 安装可被 transit 用户读取的配置到 /etc/transit/transit.toml，按访问范围设置 listen。
3. 安装 unit 到 /etc/systemd/system/transit.service，daemon-reload 后启动。
4. unit 使用 StateDirectory=transit 和 XDG_STATE_HOME=/var/lib，日志及审计数据库都在 /var/lib/transit；修改日志目录需同步调整 ReadWritePaths。
5. 使用 `sudo systemctl enable --now transit` 开机自启。模板 TimeoutStopSec=210 秒，覆盖默认 180 秒 drain；调整应用 grace 时同步提高停止等待。配置替换后 reload，监听等基础设置修改后 restart。更新程序同样使用同目录暂存、校验、原子替换和 restart 的流程。

### 容器

[Dockerfile](../../Dockerfile) 使用非 root transit 用户。构建上下文为 transit 项目目录：

```bash
docker build -t transit .
docker run --name transit --stop-timeout 210 \
  -p 8787:8787 \
  -v /absolute/config-directory:/etc/transit:ro \
  -v transit-state:/var/lib/transit \
  transit
```

配置中 listen 应为 0.0.0.0:8787，宿主机发布范围按需要约束。挂载整个配置目录以支持原子替换后 reload；单文件 bind mount 可能仍指向旧 inode。状态卷须可被容器 transit 用户写入，bind mount 预先设置 ownership，多实例不用同一日志目录、抓取目录或数据库；挂载整个 /var/lib/transit 同时持久化审计数据库、抓取和日志。

Transit 不添加调用方鉴权。网络入口控制可达范围，跨不可信网络使用 TLS；反向代理保留上游认证头、关闭 SSE buffering，并设置足够的流式超时。

## 验证边界

当前通过格式、Clippy、构建、配置校验和少量定向回归，用户联调确认 Agent 与 Edit 端到端可用。本轮测试使用本地上游夹具，验证原字节 SSE、两类协议用量、业务错误、客户端取消、观察超限、准入拒绝及 SQLite 恢复/写入故障；另验证四种压缩、组合编码、错误媒体类型、旁路容量/字节预算及持有 SQLite 写锁时响应仍能先完成。抓取验证压缩原字节、限额/队列/存储错误、权限及头过滤、清理边界、热加载与流式 drain，不调用真实模型。私有完整抓取可通过设置 TRANSIT_REPLAY_DIRECTORY 并执行 `cargo test observation::tests::private_capture_replay -- --ignored --nocapture` 离线回放，用 TRANSIT_REPLAY_EXPECTED_TOKENS 校验预期总量；夹具不进入仓库。

尚未验证 Docker/systemd 实际安装、跨平台发布产物、跨日/14 天清理边界、日志故障注入、持续负载和完整 HTTP/SSE 一致性矩阵。这些是专项验收项，不能由健康 200 或一次成功补全替代。
