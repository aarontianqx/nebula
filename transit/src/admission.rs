use crate::config::{Admission, Protocol, model_name};
use axum::http::{HeaderMap, Method, header};
use serde_json::Value;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub struct Gate {
    window: Mutex<(Instant, u32)>,
    pub rejected: AtomicU64,
    reported: AtomicU64,
}
impl Gate {
    pub fn new() -> Self {
        Self {
            window: Mutex::new((Instant::now(), 0)),
            rejected: AtomicU64::new(0),
            reported: AtomicU64::new(0),
        }
    }
    pub fn rate_allowed(&self, config: &Admission) -> bool {
        if !config.strict {
            return true;
        }
        let mut window = self.window.lock().unwrap_or_else(|e| e.into_inner());
        if window.0.elapsed() >= Duration::from_secs(1) {
            *window = (Instant::now(), 0);
        }
        if window.1 >= config.requests_per_second {
            return false;
        }
        window.1 += 1;
        true
    }
    pub fn reject(&self) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
    }
    pub fn report(&self) {
        let count = self.rejected.load(Ordering::Relaxed);
        let last = self.reported.swap(count, Ordering::Relaxed);
        if count > last {
            tracing::info!(
                event = "admission_rejections",
                count = count - last,
                total = count
            );
        }
    }
}
pub fn headers_rejection(
    config: &Admission,
    protocol: Protocol,
    method: &Method,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Option<&'static str> {
    if !config.strict {
        return None;
    }
    if protocol == Protocol::Raw {
        return Some("unsupported_protocol");
    }
    if headers.contains_key(header::UPGRADE) || *method == Method::CONNECT {
        return Some("unsupported_transport");
    }
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| {
            s.split_once(' ').is_some_and(|(scheme, key)| {
                scheme.eq_ignore_ascii_case("bearer")
                    && !key.is_empty()
                    && !key.chars().any(char::is_whitespace)
            })
        });
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| !s.is_empty() && !s.chars().any(char::is_whitespace));
    let google_key = (protocol.is_gemini() || protocol == Protocol::Models)
        && (headers
            .get("x-goog-api-key")
            .and_then(|v| v.to_str().ok())
            .is_some_and(valid_key)
            || query_keys(query).any(|key| valid_key(&key)));
    if config.require_auth
        && !(bearer || protocol == Protocol::AnthropicMessages && api_key || google_key)
    {
        return Some("missing_or_invalid_auth");
    }
    if matches!(protocol, Protocol::Models | Protocol::GeminiModels) {
        return (*method != Method::GET).then_some("invalid_method");
    }
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .is_some_and(|s| s == "application/json" || s.ends_with("+json"));
    if *method != Method::POST {
        return Some("invalid_method");
    }
    if !json {
        return Some("invalid_content_type");
    }
    if headers
        .get(header::CONTENT_ENCODING)
        .is_some_and(|s| !s.as_bytes().eq_ignore_ascii_case(b"identity"))
    {
        return Some("unsupported_content_encoding");
    }
    None
}
fn valid_key(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}
pub fn query_keys(query: Option<&str>) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .filter(|(name, _)| name == "key")
        .map(|(_, key)| key)
}
pub fn body_allowed(protocol: Protocol, value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let valid_model = object
        .get("model")
        .and_then(Value::as_str)
        .is_some_and(model_name);
    if !protocol.is_gemini() && object.get("stream").is_some_and(|v| !v.is_boolean()) {
        return false;
    }
    let messages = || {
        object
            .get("messages")
            .and_then(Value::as_array)
            // Admission recognizes an LLM envelope; message semantics belong to the upstream.
            .is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_object))
    };
    match protocol {
        Protocol::GeminiGenerateContent | Protocol::GeminiCountTokens => {
            let request = if protocol == Protocol::GeminiCountTokens {
                object.get("generateContentRequest").unwrap_or(value)
            } else {
                value
            };
            request
                .get("contents")
                .and_then(Value::as_array)
                .is_some_and(|contents| {
                    !contents.is_empty()
                        && contents.iter().all(|content| {
                            content.is_object()
                                && content
                                    .get("role")
                                    .is_none_or(|role| role.as_str().is_some_and(|s| !s.is_empty()))
                                && content.get("parts").and_then(Value::as_array).is_some_and(
                                    |parts| !parts.is_empty() && parts.iter().all(Value::is_object),
                                )
                        })
                })
        }
        Protocol::OpenaiChat => valid_model && messages(),
        Protocol::AnthropicMessages => {
            valid_model
                && messages()
                && object
                    .get("max_tokens")
                    .and_then(Value::as_u64)
                    .is_some_and(|n| n > 0)
        }
        Protocol::OpenaiCompletions => {
            valid_model
                && object.get("prompt").is_some_and(|v| {
                    v.is_string()
                        || v.as_array().is_some_and(|a| {
                            a.iter().all(|p| {
                                p.is_string()
                                    || p.as_u64().is_some()
                                    || p.as_array().is_some_and(|tokens| {
                                        tokens.iter().all(|t| t.as_u64().is_some())
                                    })
                            })
                        })
                })
        }
        Protocol::OpenaiResponses => {
            // Continuation requests may omit model/input, and tool results use arrays rather than messages.
            let continuation = object
                .get("previous_response_id")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty());
            (valid_model || continuation && !object.contains_key("model"))
                && (object.get("input").is_some_and(|v| {
                    v.is_string() || v.as_array().is_some_and(|a| a.iter().all(Value::is_object))
                }) || continuation && !object.contains_key("input"))
        }
        Protocol::Models | Protocol::GeminiModels => true,
        Protocol::Raw => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn gemini_accepts_native_keys_and_content_parts_without_opening_other_protocols() {
        let config = Admission {
            strict: true,
            ..Default::default()
        };
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("x-goog-api-key", "fixture-key".parse().unwrap());
        assert!(
            headers_rejection(
                &config,
                Protocol::GeminiGenerateContent,
                &Method::POST,
                &headers,
                None
            )
            .is_none()
        );
        assert!(
            headers_rejection(&config, Protocol::OpenaiChat, &Method::POST, &headers, None)
                .is_some()
        );
        headers.remove("x-goog-api-key");
        assert!(
            headers_rejection(
                &config,
                Protocol::GeminiGenerateContent,
                &Method::POST,
                &headers,
                Some("alt=sse&key=fixture%2Dkey")
            )
            .is_none()
        );
        assert!(
            headers_rejection(
                &config,
                Protocol::OpenaiChat,
                &Method::POST,
                &headers,
                Some("key=fixture-key")
            )
            .is_some()
        );
        for query in [
            None,
            Some("key="),
            Some("key=%0A"),
            Some("api_key=fixture-key"),
        ] {
            assert!(
                headers_rejection(
                    &config,
                    Protocol::GeminiGenerateContent,
                    &Method::POST,
                    &headers,
                    query
                )
                .is_some()
            );
        }
        let contents = json!([{"parts":[{"text":"hello"},{"inlineData":{"mimeType":"image/png","data":"AA=="}},{"functionResponse":{"name":"tool","response":{"ok":true}}}]}]);
        assert!(body_allowed(
            Protocol::GeminiGenerateContent,
            &json!({"contents":contents,"tools":[{"functionDeclarations":[]}],"generationConfig":{"temperature":0.7}})
        ));
        assert!(body_allowed(
            Protocol::GeminiCountTokens,
            &json!({"generateContentRequest":{"contents":contents}})
        ));
        for body in [
            json!({"contents":[]}),
            json!({"contents":[{"parts":[]}]}),
            json!({"contents":[{"parts":["text"]}]}),
            json!({"model":"fixture","messages":[{"role":"user","content":"hello"}]}),
        ] {
            assert!(!body_allowed(Protocol::GeminiGenerateContent, &body));
        }
    }
    #[test]
    fn strict_mode_bounds_admission_rate_without_restricting_permissive_mode() {
        let gate = Gate::new();
        let mut config = Admission {
            strict: true,
            requests_per_second: 2,
            ..Default::default()
        };
        assert!(gate.rate_allowed(&config));
        assert!(gate.rate_allowed(&config));
        assert!(!gate.rate_allowed(&config));
        config.strict = false;
        assert!(gate.rate_allowed(&config));
    }
    #[test]
    fn envelopes_allow_extensions_and_tool_continuations() {
        assert!(body_allowed(
            Protocol::OpenaiChat,
            &json!({"model":"fixture","messages":[{"role":"tool","tool_call_id":"call","content":"ok"}],"vendor_extension":{"x":true}})
        ));
        assert!(body_allowed(
            Protocol::OpenaiResponses,
            &json!({"previous_response_id":"resp_fixture","input":[{"type":"function_call_output","call_id":"call","output":"ok"}]})
        ));
        assert!(body_allowed(
            Protocol::OpenaiCompletions,
            &json!({"model":"fixture","prompt":[1,2,3]})
        ));
        assert!(!body_allowed(
            Protocol::AnthropicMessages,
            &json!({"model":"fixture","messages":[{"role":"user","content":"hello"}]})
        ));
        assert!(!body_allowed(
            Protocol::OpenaiChat,
            &json!({"model":"fixture","messages":[],"stream":"true"})
        ));
    }
    #[test]
    fn authorization_is_presence_validation_not_local_key_storage() {
        let config = Admission {
            strict: true,
            ..Default::default()
        };
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        assert!(
            headers_rejection(&config, Protocol::OpenaiChat, &Method::POST, &headers, None)
                .is_some()
        );
        headers.insert("x-api-key", "client-key".parse().unwrap());
        assert!(
            headers_rejection(
                &config,
                Protocol::AnthropicMessages,
                &Method::POST,
                &headers,
                None
            )
            .is_none()
        );
        assert!(
            headers_rejection(&config, Protocol::OpenaiChat, &Method::POST, &headers, None)
                .is_some()
        );
        headers.insert("authorization", "Bearer client-key".parse().unwrap());
        assert!(
            headers_rejection(&config, Protocol::OpenaiChat, &Method::POST, &headers, None)
                .is_none()
        );
        assert!(headers_rejection(&config, Protocol::Raw, &Method::POST, &headers, None).is_some());
        headers.insert("content-encoding", "gzip".parse().unwrap());
        assert!(
            headers_rejection(&config, Protocol::OpenaiChat, &Method::POST, &headers, None)
                .is_some()
        );
    }
}
