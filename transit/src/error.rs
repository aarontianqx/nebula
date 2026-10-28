use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ProxyError {
    pub status: StatusCode,
    pub code: &'static str,
    pub stage: &'static str,
    pub message: String,
}
impl ProxyError {
    pub fn new(
        status: StatusCode,
        code: &'static str,
        stage: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            code,
            stage,
            message: message.into(),
        }
    }
    pub fn bad(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, "request_decode", message)
    }
    pub fn upstream(code: &'static str, stage: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, code, stage, message)
    }
    pub fn timeout(stage: &'static str, seconds: u64) -> Self {
        let request = matches!(stage, "request_decode" | "request_upload");
        Self::new(
            if request {
                StatusCode::REQUEST_TIMEOUT
            } else {
                StatusCode::GATEWAY_TIMEOUT
            },
            if request {
                "request_timeout"
            } else {
                "upstream_timeout"
            },
            stage,
            format!("No progress within {seconds} seconds"),
        )
    }
}
impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        (self.status,Json(serde_json::json!({"error":{"type":"transit_error","code":self.code,"message":self.message}}))).into_response()
    }
}
