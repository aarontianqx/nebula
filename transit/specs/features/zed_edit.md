# Zed Edit Prediction 转换

Zed Agent 使用普通模型入口；Edit Prediction 使用完整的 Legacy Completions URL，发送 prompt，读取 choices[0].text。Chat 上游使用 messages、返回 choices[0].message.content。Transit 用独立路由适配二者，不把所有 `/v1/completions` 请求都强制转换。

客户端配置与触发方法见 [README](../../README.md#zed-接入)，服务端规则见 [配置模板](../../transit.example.toml)。provider 决定上游与模型别名，upstream_path 决定完整 Chat endpoint；Zed 本身不是额外 provider。

## 请求参数

转换器为 `adapter = "completions_to_chat"`，只能绑定精确 POST 路径，必须指定 prompt_mode。读取受路由 max_request_bytes 和全局上传上限约束，只接受未压缩 JSON。

| 参数 | 行为 |
| --- | --- |
| model | 必须为合法字符串，先按选定 provider 的别名映射 |
| prompt | 必须为字符串，不支持数组、token IDs |
| stream | 布尔值，省略为 false，不强迫上游流式 |
| max_tokens | 可选正整数，直接传给 Chat 上游 |
| temperature、top_p | 可选数字，范围分别为 0～2、0～1 |
| stop | 可选字符串或最多 4 个字符串组成的数组，原样传递 |
| n、echo | 仅支持省略、n=1、echo=false，转换时删除 |
| 其它参数 | 返回 400，当前 unknown_fields 仅支持 reject，无字段白名单或脚本策略 |

上游不接受 max_tokens、stop 或需要特定 reasoning 参数时直接传播错误，当前不自动改成 max_completion_tokens，也不猜测或删除参数。

转换后清除失效的长度、摘要、签名等表示 headers，设置 application/json，按 stream 选择 Accept，并要求上游 Accept-Encoding: identity。认证仍由客户端透传。

## Prompt 模式

`prompt_mode = "raw_user"` 将原始 prompt 放进一条 user message，仅承诺字段/协议转换，模型需自行理解 prompt。

`prompt_mode = "fim_qwen"` 要求以下插入式补全格式：

```text
<|fim_prefix|>{光标前代码}<|fim_suffix|>{光标后代码}<|fim_middle|>
```

将前后文作为 JSON 字符串放入 user message，通过 system 指令要求只输出中间待插入代码，保留缩进，不重复上下文或输出围栏/解释。缺失或歧义标记返回 400。

qwen 是代码上下文编码方式，不限定上游模型厂商。Chat 模型未必经过 FIM 训练，协议成功不能保证预测质量；代理不 trim 输出、不自动剥围栏或删除正文。

Zed 的 prompt_format 在客户端决定 prompt 如何生成，并不作为独立字段传给 Transit：

| Zed 格式 | 当前适配 |
| --- | --- |
| qwen | 与 fim_qwen 明确匹配，当前推荐 |
| code_gemma | 使用相同 FIM 标记，解析兼容，没有做真实客户端验收 |
| infer | Zed 按已知模型名推断，未知名字可能无法生成预测；自定义模型别名不能依赖这一识别表，也不能靠服务端映射修复 |
| star_coder、code_llama、deepseek_coder、codestral、glm | 标记不同，需要新增解析，当前 fim_qwen 拒绝 |
| Zeta/Sweep 系列 | 可能涉及替换窗口等不同输出语义，需要专门 adapter |

上游原生支持 FIM 和 `/v1/completions` 时，可按其模型要求选模板并直接透传，无需本转换器。参考 [Zed 官方文档](https://zed.dev/docs/edit-prediction#openai-compatible-servers) 和 [模板推断源码](https://github.com/zed-industries/zed/blob/main/crates/edit_prediction/src/fim.rs)。Zed 升级后需重新核对客户端行为。

## JSON 与 SSE 响应

上游非 2xx 不进入成功转换器，直接透传。成功响应必须使用匹配的 JSON/SSE Content-Type；压缩成功响应在下游响应头发送前拒绝为 502。

非流式转换有界读取并校验：

- object 改为 text_completion，message.content 映射为 choices[0].text。
- 要求恰好一个 index=0 的 choice 和最终 finish_reason（stop/length）。
- 保留可用的 id、created、model、usage、system_fingerprint；model 为真实上游名，不合成 usage。
- 工具调用、refusal、非文本或无终止原因返回 502。空文本可以合法，但有 reasoning 而无正文时拒绝作为成功结果。

流式转换增量读取 SSE，事件上限由 max_sse_event_bytes 控制：

- 支持跨 chunk UTF-8、LF/CRLF、多行 data 和注释 heartbeat，不缓冲整个回答。
- delta.content 转为 choices[0].text，role-only 和 reasoning 不作为补全文本。
- 保留 finish_reason 和合法 usage-only 帧，不支持的 choice、delta 字段、tool/refusal 或事件 schema 终止流。
- 必须先有有效 finish_reason，再收到 DONE 才成功，EOF 不补 DONE。reasoning-only 流拒绝，真正的空补全允许。
- 流内 error 事件记录脱敏摘要后终止，不伪装成空文本成功；HTTP 200 已发送时错误通过连接/流失败体现。

## 验证状态与体验

实际联调已确认 Zed Agent 默认路径、Zed Edit 独立路径和 provider 级模型映射可用，并有成功转换的 200 响应。响应时间取决于上游与负载，协议成功也不能证明预测被 Zed 展示或采纳。

输入变化可能触发新预测并取消旧请求。普通客户端取消记录为 DEBUG，不能当作上游错误；有 HTTP 错误或真正超时仍保留 WARN/ERROR。

本地回归覆盖 Qwen 前后文保留、reasoning-only/空补全边界，以及 HTTP 定长响应结束记账与错误摘要。完整 SSE 切分/异常矩阵、其它模板、持续运行和不同 Zed 版本仍需专项验收。
