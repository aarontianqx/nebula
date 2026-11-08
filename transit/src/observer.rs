use crate::{
    config::{Audit, Protocol, model_name},
    privacy::scrub,
};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Default, Serialize)]
pub struct Observation {
    pub response_model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub usage_json: Option<String>,
    pub usage_status: String,
    pub observer_status: String,
    pub observer_error: Option<String>,
    pub finish_reason: Option<String>,
    pub response_status: Option<String>,
    pub error_source: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
#[derive(Clone, Copy)]
enum Format {
    Probe,
    Json,
    Sse,
    Unsupported,
}

pub struct Observer {
    protocol: Protocol,
    result: Observation,
    format: Format,
    limit: usize,
    event_limit: usize,
    buffer: Vec<u8>,
    line: Vec<u8>,
    data: Vec<u8>,
    event_bytes: usize,
    saw_usage: bool,
    terminal: bool,
    failed: bool,
    secrets: Vec<String>,
    usage: Map<String, Value>,
}
impl Observer {
    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len() + self.line.len() + self.data.len()
    }
    pub fn stopped(&self) -> bool {
        self.failed || matches!(self.format, Format::Unsupported)
    }
    pub fn new(protocol: Protocol, settings: &Audit, secrets: Vec<String>) -> Self {
        Self {
            protocol,
            result: Observation {
                usage_status: "missing".into(),
                observer_status: if protocol.generates() {
                    "pending"
                } else {
                    "unsupported"
                }
                .into(),
                ..Default::default()
            },
            format: Format::Unsupported,
            limit: settings.max_response_json_bytes,
            event_limit: settings.max_sse_event_bytes,
            buffer: Vec::new(),
            line: Vec::new(),
            data: Vec::new(),
            event_bytes: 0,
            saw_usage: false,
            terminal: false,
            failed: false,
            secrets,
            usage: Map::new(),
        }
    }
    pub fn headers(&mut self, _media: &str, identity: bool) {
        // Only recognized generation protocols are inspected. The initial body
        // determines framing, so JSON errors and mislabelled SSE both work.
        self.format = if identity && self.protocol.generates() {
            Format::Probe
        } else {
            Format::Unsupported
        };
        self.result.observer_status = if matches!(self.format, Format::Unsupported) {
            "unsupported"
        } else {
            "observing"
        }
        .into();
    }
    pub fn unsupported(&mut self, code: &str) {
        self.format = Format::Unsupported;
        self.result.observer_status = "unsupported".into();
        self.result.observer_error = Some(code.into());
        self.buffer = Vec::new();
    }
    pub fn degrade(&mut self, code: &str) {
        self.failed = true;
        self.result.observer_status = "parse_error".into();
        self.result.observer_error = Some(code.into());
        self.buffer = Vec::new();
        self.line = Vec::new();
        self.data = Vec::new();
    }
    pub fn bytes(&mut self, chunk: &[u8]) {
        if self.failed {
            return;
        }
        if matches!(self.format, Format::Probe) {
            const PROBE_LIMIT: usize = 8192;
            let count = chunk
                .len()
                .min(PROBE_LIMIT.saturating_sub(self.buffer.len()));
            self.buffer.extend_from_slice(&chunk[..count]);
            let mut prefix = self.buffer.as_slice();
            if prefix.starts_with(&[0xef, 0xbb, 0xbf]) {
                prefix = &prefix[3..];
            }
            prefix = prefix.trim_ascii_start();
            let format = if prefix.first().is_some_and(|b| matches!(b, b'{' | b'[')) {
                Some(Format::Json)
            } else if prefix.starts_with(b":")
                || [b"data:".as_slice(), b"event:", b"id:", b"retry:"]
                    .iter()
                    .any(|p| prefix.starts_with(p))
            {
                Some(Format::Sse)
            } else {
                None
            };
            if let Some(format) = format {
                self.format = format;
                let mut prefix = std::mem::take(&mut self.buffer);
                if prefix.starts_with(&[0xef, 0xbb, 0xbf]) {
                    prefix.drain(..3);
                }
                self.body_bytes(&prefix);
                if !self.failed {
                    self.body_bytes(&chunk[count..]);
                }
            } else if self.buffer.len() == PROBE_LIMIT {
                self.unsupported("unrecognized_response_format");
            }
            return;
        }
        self.body_bytes(chunk);
    }
    fn body_bytes(&mut self, chunk: &[u8]) {
        match self.format {
            Format::Probe | Format::Unsupported => {}
            Format::Json => {
                if self.buffer.len().saturating_add(chunk.len()) > self.limit {
                    self.degrade("json_limit");
                } else {
                    self.buffer.extend_from_slice(chunk);
                }
            }
            Format::Sse => {
                for byte in chunk {
                    self.event_bytes = self.event_bytes.saturating_add(1);
                    if self.event_bytes > self.event_limit {
                        self.degrade("sse_event_limit");
                        break;
                    }
                    if *byte != b'\n' {
                        self.line.push(*byte);
                        continue;
                    }
                    let mut line = std::mem::take(&mut self.line);
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    if line.is_empty() {
                        let data = std::mem::take(&mut self.data);
                        self.event_bytes = 0;
                        if data == b"[DONE]" {
                            self.terminal = true;
                        } else if !data.is_empty() {
                            match serde_json::from_slice::<Value>(&data) {
                                Ok(value) => self.value(&value, true),
                                Err(_) => {
                                    self.degrade("invalid_sse_json");
                                    break;
                                }
                            }
                        }
                    } else if let Some(data) = line.strip_prefix(b"data:") {
                        if !self.data.is_empty() {
                            self.data.push(b'\n');
                        }
                        self.data
                            .extend_from_slice(data.strip_prefix(b" ").unwrap_or(data));
                    }
                }
            }
        }
    }
    pub fn value(&mut self, value: &Value, streaming: bool) {
        if !value.is_object() {
            self.degrade("invalid_json_object");
            return;
        }
        let event = value.get("type").and_then(Value::as_str).unwrap_or("");
        let object = if matches!(self.protocol, Protocol::OpenaiResponses) {
            value
                .get("response")
                .filter(|v| v.is_object())
                .unwrap_or(value)
        } else if self.protocol == Protocol::AnthropicMessages && event == "message_start" {
            value
                .get("message")
                .filter(|v| v.is_object())
                .unwrap_or(value)
        } else {
            value
        };
        if let Some(model) = object
            .get(if self.protocol == Protocol::GeminiGenerateContent {
                "modelVersion"
            } else {
                "model"
            })
            .and_then(Value::as_str)
            .filter(|m| model_name(m))
        {
            self.result.response_model = Some(scrub(model, &self.secrets, 256));
        }
        if self.protocol == Protocol::GeminiGenerateContent {
            if let Some(usage) = object.get("usageMetadata").and_then(Value::as_object) {
                for key in [
                    "promptTokenCount",
                    "candidatesTokenCount",
                    "totalTokenCount",
                    "cachedContentTokenCount",
                    "thoughtsTokenCount",
                    "toolUsePromptTokenCount",
                ] {
                    if let Some(number) = usage
                        .get(key)
                        .filter(|v| v.as_u64().is_some_and(|n| n <= i64::MAX as u64))
                    {
                        self.usage.insert(key.into(), number.clone());
                        self.saw_usage = true;
                    }
                }
            }
            if let Some(reason) = object
                .pointer("/candidates/0/finishReason")
                .or_else(|| object.pointer("/promptFeedback/blockReason"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                self.result.finish_reason = Some(scrub(reason, &self.secrets, 128));
                self.terminal = true;
            }
        }
        // Only numeric usage fields are retained: arbitrary provider extensions may contain user data.
        if let Some(usage) = object.get("usage").and_then(Value::as_object) {
            for (key, value) in usage {
                if matches!(
                    key.as_str(),
                    "prompt_tokens"
                        | "completion_tokens"
                        | "total_tokens"
                        | "input_tokens"
                        | "output_tokens"
                        | "cache_read_input_tokens"
                        | "cache_creation_input_tokens"
                ) && value.as_u64().is_some_and(|n| n <= i64::MAX as u64)
                {
                    self.usage.insert(key.clone(), value.clone());
                    self.saw_usage = true;
                }
                if matches!(
                    key.as_str(),
                    "prompt_tokens_details"
                        | "completion_tokens_details"
                        | "input_tokens_details"
                        | "output_tokens_details"
                        | "cache_creation"
                ) && let Some(details) = value.as_object()
                {
                    let target = self
                        .usage
                        .entry(key.clone())
                        .or_insert_with(|| Value::Object(Map::new()));
                    if let Some(target) = target.as_object_mut() {
                        for (field, number) in details {
                            if matches!(
                                field.as_str(),
                                "cached_tokens"
                                    | "cache_write_tokens"
                                    | "reasoning_tokens"
                                    | "thinking_tokens"
                                    | "audio_tokens"
                                    | "text_tokens"
                                    | "image_tokens"
                                    | "accepted_prediction_tokens"
                                    | "rejected_prediction_tokens"
                                    | "ephemeral_5m_input_tokens"
                                    | "ephemeral_1h_input_tokens"
                            ) && number.as_u64().is_some_and(|n| n <= i64::MAX as u64)
                            {
                                target.insert(field.clone(), number.clone());
                            }
                        }
                    }
                }
            }
        }
        if let Some(reason) = object
            .pointer("/choices/0/finish_reason")
            .or_else(|| object.get("stop_reason"))
            .or_else(|| value.pointer("/delta/stop_reason"))
            .and_then(Value::as_str)
        {
            self.result.finish_reason = Some(scrub(reason, &self.secrets, 128));
        }
        if let Some(status) = object.get("status").and_then(Value::as_str).filter(|s| {
            matches!(
                *s,
                "completed" | "failed" | "incomplete" | "in_progress" | "queued" | "cancelled"
            )
        }) {
            self.result.response_status = Some(status.into());
            if matches!(status, "completed" | "failed" | "incomplete" | "cancelled") {
                self.terminal = true;
            }
        }
        if event == "message_stop" || event == "error" {
            self.terminal = true;
        }
        if let Some(error) = object
            .get("error")
            .filter(|v| !v.is_null())
            .or_else(|| value.get("error").filter(|v| !v.is_null()))
            .or_else(|| (event == "error").then_some(value))
        {
            self.result.error_source = Some("upstream_body".into());
            self.result.error_code = error
                .get("code")
                .and_then(Value::as_str)
                .or_else(|| error.get("type").and_then(Value::as_str))
                .or_else(|| error.get("status").and_then(Value::as_str))
                .map(|s| scrub(s, &self.secrets, 128));
            self.result.error_message = error
                .get("message")
                .and_then(Value::as_str)
                .map(|s| scrub(s, &self.secrets, 512));
            if self.protocol == Protocol::GeminiGenerateContent {
                self.terminal = true;
            }
        }
        if self.result.response_status.as_deref() == Some("failed")
            && self.result.error_source.is_none()
        {
            self.result.error_source = Some("upstream_body".into());
            self.result.error_code = Some("response_failed".into());
        }
        if !streaming {
            self.terminal = true;
        }
        self.update_usage();
    }
    fn update_usage(&mut self) {
        if !self.saw_usage {
            return;
        }
        let u = Value::Object(self.usage.clone());
        let get = |key: &str| u.get(key).and_then(Value::as_u64);
        let detail = |path: &str| u.pointer(path).and_then(Value::as_u64);
        match self.protocol {
            Protocol::GeminiGenerateContent => {
                self.result.input_tokens = get("promptTokenCount")
                    .and_then(|n| n.checked_add(get("toolUsePromptTokenCount").unwrap_or(0)))
                    .filter(|n| *n <= i64::MAX as u64);
                self.result.reasoning_tokens = get("thoughtsTokenCount");
                self.result.output_tokens = get("candidatesTokenCount")
                    .and_then(|n| n.checked_add(self.result.reasoning_tokens.unwrap_or(0)))
                    .filter(|n| *n <= i64::MAX as u64);
                self.result.cache_read_tokens = get("cachedContentTokenCount");
            }
            Protocol::OpenaiChat | Protocol::OpenaiCompletions => {
                self.result.input_tokens = get("prompt_tokens");
                self.result.output_tokens = get("completion_tokens");
                self.result.cache_read_tokens = detail("/prompt_tokens_details/cached_tokens");
                self.result.cache_write_tokens =
                    detail("/prompt_tokens_details/cache_write_tokens");
                self.result.reasoning_tokens =
                    detail("/completion_tokens_details/reasoning_tokens");
            }
            Protocol::OpenaiResponses => {
                self.result.input_tokens = get("input_tokens");
                self.result.output_tokens = get("output_tokens");
                self.result.cache_read_tokens = detail("/input_tokens_details/cached_tokens");
                self.result.cache_write_tokens = detail("/input_tokens_details/cache_write_tokens");
                self.result.reasoning_tokens = detail("/output_tokens_details/reasoning_tokens");
            }
            Protocol::AnthropicMessages => {
                self.result.cache_read_tokens = get("cache_read_input_tokens");
                self.result.cache_write_tokens = get("cache_creation_input_tokens");
                self.result.input_tokens = get("input_tokens")
                    .and_then(|base| base.checked_add(self.result.cache_read_tokens.unwrap_or(0)))
                    .and_then(|n| n.checked_add(self.result.cache_write_tokens.unwrap_or(0)))
                    .filter(|n| *n <= i64::MAX as u64);
                self.result.output_tokens = get("output_tokens");
                self.result.reasoning_tokens = detail("/output_tokens_details/thinking_tokens");
            }
            _ => {}
        }
        self.result.total_tokens = get(if self.protocol == Protocol::GeminiGenerateContent {
            "totalTokenCount"
        } else {
            "total_tokens"
        })
        .or_else(|| {
            self.result
                .input_tokens
                .zip(self.result.output_tokens)
                .and_then(|(a, b)| a.checked_add(b))
                .filter(|n| *n <= i64::MAX as u64)
        });
        self.result.usage_json = Some(u.to_string());
    }
    pub fn finish(&mut self, transport_complete: bool) -> Observation {
        if !self.failed && matches!(self.format, Format::Probe) {
            self.unsupported("unrecognized_or_empty_response");
        }
        if !self.failed && matches!(self.format, Format::Json) && !self.buffer.is_empty() {
            let bytes = std::mem::take(&mut self.buffer);
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Array(values)) if self.protocol == Protocol::GeminiGenerateContent => {
                    for value in values {
                        self.value(&value, true);
                        if self.failed {
                            break;
                        }
                    }
                    if transport_complete && !self.terminal && !self.failed {
                        self.degrade("missing_terminal_event");
                    }
                }
                Ok(v) => self.value(&v, false),
                Err(_) => self.degrade("invalid_or_truncated_json"),
            }
        }
        if !self.failed
            && matches!(self.format, Format::Sse)
            && transport_complete
            && !self.terminal
        {
            self.degrade("missing_terminal_event");
        }
        if self.result.observer_status == "observing" {
            self.result.observer_status = if transport_complete {
                "observed"
            } else {
                "partial"
            }
            .into();
        }
        self.result.usage_status = if self.saw_usage {
            if transport_complete
                && self.terminal
                && !self.failed
                && self.result.input_tokens.is_some()
                && self.result.output_tokens.is_some()
            {
                "complete"
            } else {
                "partial"
            }
        } else {
            "missing"
        }
        .into();
        self.result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gemini_json_sse_and_json_array_usage_include_thinking_without_double_counting() {
        let first = serde_json::json!({"modelVersion":"fixture-native","candidates":[{"content":{"parts":[{"text":"hello"}]}}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":1}});
        let last = serde_json::json!({"modelVersion":"fixture-native","candidates":[{"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":20,"thoughtsTokenCount":5,"toolUsePromptTokenCount":3,"cachedContentTokenCount":60,"totalTokenCount":128,"privateExtension":"fixture-secret"}});
        for body in [
            last.to_string(),
            format!("data: {first}\n\ndata: {last}\n\n"),
            serde_json::json!([first, last]).to_string(),
        ] {
            let mut o = observer(Protocol::GeminiGenerateContent);
            o.headers("application/json", true);
            for part in body.as_bytes().chunks(3) {
                o.bytes(part);
            }
            let r = o.finish(true);
            assert_eq!(r.input_tokens, Some(103));
            assert_eq!(r.output_tokens, Some(25));
            assert_eq!(r.total_tokens, Some(128));
            assert_eq!(r.cache_read_tokens, Some(60));
            assert_eq!(r.cache_write_tokens, None);
            assert_eq!(r.reasoning_tokens, Some(5));
            assert_eq!(r.finish_reason.as_deref(), Some("STOP"));
            assert_eq!(r.response_model.as_deref(), Some("fixture-native"));
            assert_eq!(r.usage_status, "complete");
            assert!(!r.usage_json.unwrap().contains("privateExtension"));
        }
        let mut o = observer(Protocol::GeminiGenerateContent);
        o.headers("text/event-stream", true);
        o.bytes(format!("data: {first}\n\n").as_bytes());
        let r = o.finish(true);
        assert_eq!(r.usage_status, "partial");
        assert_eq!(r.observer_error.as_deref(), Some("missing_terminal_event"));
        let mut o = observer(Protocol::GeminiGenerateContent);
        o.headers("text/event-stream", true);
        o.bytes(b"data: {\"error\":{\"code\":429,\"status\":\"RESOURCE_EXHAUSTED\",\"message\":\"retry fixture-secret\"}}\n\n");
        let r = o.finish(true);
        assert_eq!(r.error_code.as_deref(), Some("RESOURCE_EXHAUSTED"));
        assert!(!r.error_message.unwrap().contains("fixture-secret"));
        assert!(r.observer_error.is_none());
    }
    use serde_json::json;
    fn observer(protocol: Protocol) -> Observer {
        Observer::new(protocol, &Audit::default(), vec!["fixture-secret".into()])
    }
    fn sse(observer: &mut Observer, value: Value) {
        let bytes = format!("data: {value}\r\n\r\n");
        for chunk in bytes.as_bytes().chunks(3) {
            observer.bytes(chunk);
        }
    }
    #[test]
    fn chat_usage_does_not_double_count_cached_or_reasoning_tokens() {
        for protocol in [Protocol::OpenaiChat, Protocol::OpenaiCompletions] {
            let mut o = observer(protocol);
            o.headers("text/event-stream", true);
            sse(
                &mut o,
                json!({"choices":[{"delta":{"content":"private response"}}],"usage":null}),
            );
            sse(
                &mut o,
                json!({"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"prompt_tokens_details":{"cached_tokens":80,"cache_write_tokens":10},"completion_tokens_details":{"reasoning_tokens":12},"custom":"private input"}}),
            );
            o.bytes(b"data: [DONE]\n\n");
            let r = o.finish(true);
            assert_eq!(r.input_tokens, Some(100));
            assert_eq!(r.output_tokens, Some(20));
            assert_eq!(r.total_tokens, Some(120));
            assert_eq!(r.reasoning_tokens, Some(12));
            assert_eq!(r.cache_read_tokens, Some(80));
            assert_eq!(r.cache_write_tokens, Some(10));
            assert_eq!(r.usage_status, "complete");
            assert!(!serde_json::to_string(&r).unwrap().contains("private"));
        }
    }
    #[test]
    fn responses_json_and_sse_use_input_details_and_preserve_unknown_and_zero_cache_writes() {
        for write in [None, Some(0), Some(10)] {
            let mut response = json!({"status":"completed","usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120,"input_tokens_details":{"cached_tokens":80},"output_tokens_details":{"reasoning_tokens":12}}});
            if let Some(write) = write {
                response["usage"]["input_tokens_details"]["cache_write_tokens"] = json!(write);
            }
            for streaming in [false, true] {
                let mut o = observer(Protocol::OpenaiResponses);
                o.headers("application/json", true);
                if streaming {
                    sse(
                        &mut o,
                        json!({"type":"response.completed","response":response}),
                    );
                } else {
                    o.bytes(response.to_string().as_bytes());
                }
                let r = o.finish(true);
                assert_eq!(r.input_tokens, Some(100));
                assert_eq!(r.output_tokens, Some(20));
                assert_eq!(r.total_tokens, Some(120));
                assert_eq!(r.cache_read_tokens, Some(80));
                assert_eq!(r.cache_write_tokens, write);
                assert_eq!(r.reasoning_tokens, Some(12));
                assert_eq!(r.usage_status, "complete");
                let stored = serde_json::to_value(&r).unwrap();
                assert!(stored.get("cached_input_tokens").is_none());
                assert!(stored.get("cache_write_input_tokens").is_none());
                let usage: Value = serde_json::from_str(r.usage_json.as_ref().unwrap()).unwrap();
                assert_eq!(
                    usage["input_tokens_details"]["cache_write_tokens"].as_u64(),
                    write
                );
            }
        }
    }
    #[test]
    fn anthropic_cumulative_updates_merge_and_input_includes_cache() {
        let mut o = observer(Protocol::AnthropicMessages);
        o.headers("text/event-stream", true);
        sse(
            &mut o,
            json!({"type":"message_start","message":{"model":"fixture-model","usage":{"input_tokens":10,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":0}}}),
        );
        sse(
            &mut o,
            json!({"type":"message_delta","usage":{"output_tokens":5}}),
        );
        sse(
            &mut o,
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7}}),
        );
        sse(&mut o, json!({"type":"future_event","payload":"private"}));
        sse(&mut o, json!({"type":"message_stop"}));
        let r = o.finish(true);
        assert_eq!(r.input_tokens, Some(60));
        assert_eq!(r.output_tokens, Some(7));
        assert_eq!(r.total_tokens, Some(67));
        assert_eq!(r.cache_read_tokens, Some(30));
        assert_eq!(r.cache_write_tokens, Some(20));
        assert_eq!(r.usage_status, "complete");
        assert_eq!(r.finish_reason.as_deref(), Some("end_turn"));
    }
    #[test]
    fn response_failure_inside_http_success_is_visible_and_redacted() {
        let mut o = observer(Protocol::OpenaiResponses);
        o.headers("text/event-stream", true);
        sse(
            &mut o,
            json!({"type":"response.failed","response":{"status":"failed","error":{"code":"engine_overloaded","message":"try later fixture-secret"},"usage":{"input_tokens":20,"output_tokens":3,"input_tokens_details":{"cached_tokens":10},"output_tokens_details":{"reasoning_tokens":2}}}}),
        );
        let r = o.finish(true);
        assert_eq!(r.error_source.as_deref(), Some("upstream_body"));
        assert_eq!(r.total_tokens, Some(23));
        assert!(!r.error_message.unwrap().contains("fixture-secret"));
    }
    #[test]
    fn missing_partial_and_over_limit_usage_remain_distinct() {
        let mut o = observer(Protocol::OpenaiChat);
        o.headers("text/event-stream", true);
        o.bytes(b"data: [DONE]\n\n");
        let r = o.finish(true);
        assert_eq!(r.usage_status, "missing");
        assert_eq!(r.input_tokens, None);
        let mut o = observer(Protocol::AnthropicMessages);
        o.headers("text/event-stream", true);
        sse(
            &mut o,
            json!({"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":0}}}),
        );
        let r = o.finish(false);
        assert_eq!(r.usage_status, "partial");
        assert_eq!(r.input_tokens, Some(10));
        let mut o = observer(Protocol::OpenaiChat);
        o.headers("text/event-stream", true);
        o.event_limit = 16;
        o.bytes(b"data: {\"very_large_event\":1}\n\n");
        let r = o.finish(true);
        assert_eq!(r.observer_status, "parse_error");
        assert_eq!(r.observer_error.as_deref(), Some("sse_event_limit"));
        assert_eq!(r.usage_status, "missing");
    }
    #[test]
    fn unsupported_and_invalid_json_do_not_invent_usage() {
        let mut o = observer(Protocol::OpenaiChat);
        o.headers("application/json", false);
        o.bytes(b"compressed");
        let r = o.finish(true);
        assert_eq!(r.observer_status, "unsupported");
        assert!(r.input_tokens.is_none());
        let mut o = observer(Protocol::OpenaiChat);
        o.headers("application/json", true);
        o.bytes(b"{broken");
        let r = o.finish(true);
        assert_eq!(r.observer_status, "parse_error");
        assert!(r.usage_json.is_none());
    }
    #[test]
    fn standalone_response_errors_and_large_counters_are_safe() {
        let mut o = observer(Protocol::OpenaiResponses);
        o.headers("text/event-stream", true);
        sse(
            &mut o,
            json!({"type":"error","code":"server_error","message":"retry later"}),
        );
        let r = o.finish(true);
        assert_eq!(r.error_code.as_deref(), Some("server_error"));
        assert_eq!(r.error_source.as_deref(), Some("upstream_body"));
        let mut o = observer(Protocol::AnthropicMessages);
        o.headers("application/json", true);
        o.bytes(json!({"usage":{"input_tokens":i64::MAX,"output_tokens":1,"cache_read_input_tokens":1}}).to_string().as_bytes());
        let r = o.finish(true);
        assert!(r.input_tokens.is_none());
        assert!(r.total_tokens.is_none());
        assert_eq!(r.usage_status, "partial");
        let mut o = observer(Protocol::Raw);
        o.headers("application/json", true);
        o.bytes(b"{vendor_payload");
        assert_eq!(o.finish(true).observer_status, "unsupported");
    }
}
