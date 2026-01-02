//! OCR service client infrastructure.
//!
//! Provides OCR recognition services via HTTP calls to a FastAPI backend.
//! Uses a global singleton to avoid creating multiple health check threads.

mod client;
mod config;

use std::sync::OnceLock;

pub use client::{HttpOcrClient, NoOpOcrClient, OcrClient, OcrClientHandle, Roi, UsageRatioResult};
pub use config::OcrConfig;

/// Global OCR client singleton
static GLOBAL_OCR_CLIENT: OnceLock<OcrClientHandle> = OnceLock::new();

/// Get or initialize the global OCR client.
/// This is a singleton - only one health check thread runs for the entire application.
pub fn global_ocr_client() -> OcrClientHandle {
    GLOBAL_OCR_CLIENT
        .get_or_init(|| {
            tracing::info!("Initializing global OCR client");
            std::sync::Arc::new(HttpOcrClient::new(OcrConfig::default()))
        })
        .clone()
}
