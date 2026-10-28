# 运行与运维

启动、构建和配置入口见 [README](../../README.md)。本文维护目录、日志、健康、reload 和部署契约。

## 配置与日志目录

源码目录为仓库内独立的 transit/。运行数据不随 cwd 写入源码或 target。

| 环境 | 配置默认位置 | 日志默认位置 |
| --- | --- | --- |
| Linux 用户进程 | `$XDG_CONFIG_HOME/transit/transit.toml`，未设置时 `~/.config/transit/transit.toml` | `$XDG_STATE_HOME/transit/logs`，未设置时 `~/.local/state/transit/logs` |
| macOS 服务进程 | `~/.config/transit/transit.toml` | `~/Library/Logs/transit` |
| 附带 systemd/容器模板 | 显式指定 `/etc/transit/transit.toml` | XDG_STATE_HOME=/var/lib，对应 `/var/lib/transit/logs` |

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
| WARN | 上游 4xx、本地参数拒绝、无效 reload、容量拒绝、无法分类的连接失败、清理失败 |
| ERROR | 上游 5xx、连接/超时、转换/截断、内部任务或日志基础设施错误 |

request_id 是 Transit 生成的 UUID，经 X-Transit-Request-Id 返回。日志按需包含 provider、rule_id、config_generation、method、adapter、请求/实际模型、状态、outcome、error_code、stage、耗时、首个上游 body 字节时间和上下行字节数。

model 只在原本已解析 JSON 的请求中提取，不为了日志解析透传 body。response_bytes 是已读取的上游 body 字节数，adapter 转换后客户端收到的大小可能不同。completed 表示数据已交给 HTTP transport，不保证客户端应用读取或接受。

运行日志会包含实际 provider 名、请求/解析后模型名，以及启动时的配置路径、日志目录和监听地址，可能暴露部署信息或模型偏好。默认存放在仓库外的私有目录，不能作为公开示例直接提交或发布；分享排障日志前应进一步清理这些字段。示例配置、文档和测试仅使用通用 provider/model 值，不记录个人部署与联调配置。

HTTP 错误摘要使用最多 error_preview_bytes 的 body 前缀（默认 4096，0 关闭正文捕获），只提取已知 JSON error 的 type/code/param/message：

- 认证值、引号内片段、疑似 token、URL、代码/多行内容经过过滤；不记录完整 headers、query、prompt、正常响应或原始 SSE。
- 非 JSON、压缩、超限或截断无法解析时记录 unavailable 原因和受限上游 request ID，不 dump 原文。
- 有界预览不等待完整响应，已捕获摘要在定长响应完成、流失败或客户端取消时均尝试报告。
- 返回客户端时仍保留上游错误字节。透传成功 SSE 的业务错误不额外解析，adapter 内的 error 事件单独诊断。

字段过滤用于减少泄露风险，不等于允许公开日志，文件仍按私有运行数据管理。

## 日志故障可见性

tracing-appender 使用 8192 条写入的有界 lossy 队列和 16 KiB 单次事件写入上限。队列满丢日志而不阻塞模型流，通过 ErrorCounter 定期报告增量；writer 额外监测写入、flush 和日切换故障，向 stderr 告警并维护健康计数。文件恢复写入后报告 logging_recovered。

默认 WARN/ERROR 还输出 stderr，systemd/journald 或容器应采集它作为备用出口。日志故障不强杀模型请求；异步队列不承诺 SIGKILL、断电或文件/stderr 同时不可写时持久化。正常退出保留 worker guard 到最后，不逐行 fsync。

## 健康、reload 与退出

GET `/_transit/health/live` 返回 alive 和 logging_degraded、log_write_failures、log_events_dropped。GET `/_transit/health/ready` 返回 ready、config_generation、active_requests 和 logging_degraded。

健康不访问上游，也不验证 endpoint/key/model。日志故障只改变诊断字段，不使 ready 自动变为 false；draining 时 ready 返回 503。接入层应按需要读取日志健康字段。

SIGHUP 重新读取完整配置，成功后原子切换，失败保留旧快照；已有请求使用旧 generation。支持修改默认 provider、provider/别名、路由、logging.level/stderr_level/error_preview_bytes。

所有 server 参数，以及 logging.directory/rotation/retention_days/cleanup_interval_seconds 变化需要重启。如果 reload 文件包含此类修改，整份 reload 拒绝，不部分应用。没有自动文件监视。建议原子替换配置文件后再 SIGHUP；systemd 使用 `systemctl reload transit`。

SIGTERM/SIGINT 开始 drain，默认 grace=30 秒，停止接收新连接/请求，尽量完成在途请求，超时后强制结束。部署 stop timeout 应大于 grace 并留出清理余量。

## 部署

systemd 模板为 [transit.service](../../deploy/transit.service)：

1. 创建专用 transit 用户/组，安装二进制到 /usr/local/bin/transit。
2. 安装可被 transit 用户读取的配置到 /etc/transit/transit.toml，按访问范围设置 listen。
3. 安装 unit 到 /etc/systemd/system/transit.service，daemon-reload 后启动。
4. unit 使用 StateDirectory=transit 和 XDG_STATE_HOME=/var/lib，日志为 /var/lib/transit/logs；修改日志目录需同步调整 ReadWritePaths。
5. 修改 shutdown grace 后同步调整 TimeoutStopSec。配置替换后 reload，监听等基础设置修改后 restart。

[Dockerfile](../../Dockerfile) 使用非 root transit 用户。构建上下文为 transit 项目目录：

```bash
docker build -t transit .
docker run --name transit --stop-timeout 40 \
  -p 8787:8787 \
  -v /absolute/config-directory:/etc/transit:ro \
  -v transit-logs:/var/lib/transit/logs \
  transit
```

配置中 listen 应为 0.0.0.0:8787，宿主机发布范围按需要约束。挂载整个配置目录以支持原子替换后 reload；单文件 bind mount 可能仍指向旧 inode。日志卷须可被容器 transit 用户写入，bind mount 预先设置 ownership，多实例不用同一日志目录。

Transit 不添加调用方鉴权。网络入口控制可达范围，跨不可信网络使用 TLS；反向代理保留上游认证头、关闭 SSE buffering，并设置足够的流式超时。

## 验证边界

当前通过格式、Clippy、构建、配置校验和少量定向回归，用户联调确认 Agent 与 Edit 端到端可用。本轮 review 不调用真实模型。

尚未验证 Docker/systemd 实际安装、跨平台发布产物、跨日/14 天清理边界、日志故障注入、持续负载和完整 HTTP/SSE 一致性矩阵。这些是专项验收项，不能由健康 200 或一次成功补全替代。
