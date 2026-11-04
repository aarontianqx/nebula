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
pub fn headers_allowed(
    config: &Admission,
    protocol: Protocol,
    method: &Method,
    headers: &HeaderMap,
) -> bool {
    if !config.strict {
        return true;
    }
    if protocol == Protocol::Raw
        || headers.contains_key(header::UPGRADE)
        || *method == Method::CONNECT
    {
        return false;
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
    if config.require_auth && !(bearer || protocol == Protocol::AnthropicMessages && api_key) {
        return false;
    }
    if protocol == Protocol::Models {
        return *method == Method::GET;
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
    *method == Method::POST
        && json
        && headers
            .get(header::CONTENT_ENCODING)
            .is_none_or(|s| s.as_bytes().eq_ignore_ascii_case(b"identity"))
}
pub fn body_allowed(protocol: Protocol, value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let valid_model = object
        .get("model")
        .and_then(Value::as_str)
        .is_some_and(model_name);
    if object.get("stream").is_some_and(|v| !v.is_boolean()) {
        return false;
    }
    let messages = || {
        object
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|a| {
                !a.is_empty()
                    && a.iter().all(|m| {
                        m.is_object()
                            && m.get("role")
                                .and_then(Value::as_str)
                                .is_some_and(|s| !s.is_empty())
                            && (m
                                .get("content")
                                .is_some_and(|v| v.is_string() || v.is_array())
                                || protocol == Protocol::OpenaiChat
                                    && (m.get("tool_calls").is_some_and(Value::is_array)
                                        || m.get("function_call").is_some_and(Value::is_object)
                                        || m.get("refusal").is_some_and(Value::is_string)))
                    })
            })
    };
    match protocol {
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
        Protocol::Models => true,
        Protocol::Raw => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
        assert!(!headers_allowed(
            &config,
            Protocol::OpenaiChat,
            &Method::POST,
            &headers
        ));
        headers.insert("x-api-key", "client-key".parse().unwrap());
        assert!(headers_allowed(
            &config,
            Protocol::AnthropicMessages,
            &Method::POST,
            &headers
        ));
        assert!(!headers_allowed(
            &config,
            Protocol::OpenaiChat,
            &Method::POST,
            &headers
        ));
        headers.insert("authorization", "Bearer client-key".parse().unwrap());
        assert!(headers_allowed(
            &config,
            Protocol::OpenaiChat,
            &Method::POST,
            &headers
        ));
        assert!(!headers_allowed(
            &config,
            Protocol::Raw,
            &Method::POST,
            &headers
        ));
        headers.insert("content-encoding", "gzip".parse().unwrap());
        assert!(!headers_allowed(
            &config,
            Protocol::OpenaiChat,
            &Method::POST,
            &headers
        ));
    }
}
