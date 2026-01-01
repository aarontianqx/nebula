//! OCR service client infrastructure.
//!
//! Provides OCR recognition services via HTTP calls to a FastAPI backend.

mod client;
mod config;

pub use client::{HttpOcrClient, NoOpOcrClient, OcrClient, OcrClientHandle, Roi, UsageRatioResult};
pub use config::OcrConfig;
