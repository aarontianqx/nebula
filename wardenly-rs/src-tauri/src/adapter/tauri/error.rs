use crate::domain::error::DomainError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub message: String,
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        ApiError {
            message: err.to_string(),
        }
    }
}

impl From<ApiError> for String {
    fn from(err: ApiError) -> Self {
        err.message
    }
}

