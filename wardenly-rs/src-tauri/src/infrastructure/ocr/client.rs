//! OCR client implementations.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use image::DynamicImage;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::watch;
use tokio::time::sleep;

use super::config::OcrConfig;

/// Region of interest for OCR recognition.
#[derive(Debug, Clone)]
pub struct Roi {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Result of usage ratio recognition (e.g., "1/10" -> numerator=1, denominator=10).
#[derive(Debug, Clone)]
pub struct UsageRatioResult {
    pub numerator: i32,
    pub denominator: i32,
    pub raw_text: String,
    pub confidence: f64,
}

/// OCR client trait for recognizing text/ratios from images.
#[async_trait]
pub trait OcrClient: Send + Sync {
    /// Check if the OCR service is currently healthy.
    fn is_healthy(&self) -> bool;

    /// Recognize a usage ratio (e.g., "1/10") from an image.
    async fn recognize_usage_ratio(
        &self,
        image: &DynamicImage,
        roi: Option<&Roi>,
    ) -> anyhow::Result<UsageRatioResult>;

    /// Shutdown the client and any background tasks.
    fn close(&self);
}

/// Handle to an OCR client for cloning and sharing.
pub type OcrClientHandle = Arc<dyn OcrClient>;

/// HTTP-based OCR client that calls a FastAPI backend.
pub struct HttpOcrClient {
    config: OcrConfig,
    client: Arc<Client>,
    healthy: Arc<AtomicBool>,
    shutdown_tx: watch::Sender<bool>,
}

impl HttpOcrClient {
    /// Create a new HTTP OCR client with background health checking.
    pub fn new(config: OcrConfig) -> Self {
        // Use a shorter timeout for health checks, but share the same connection pool
        let client = Arc::new(
            Client::builder()
                .timeout(config.timeout())
                .pool_max_idle_per_host(2)
                .pool_idle_timeout(Some(std::time::Duration::from_secs(4))) // Close idle connections before server kills them (default ~5s)
                .build()
                .expect("Failed to create HTTP client"),
        );

        let healthy = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Start background health check loop with shared client
        let health_client = client.clone();
        let health_config = config.clone();
        let health_healthy = healthy.clone();
        tokio::spawn(async move {
            Self::health_check_loop(health_client, health_config, health_healthy, shutdown_rx).await;
        });

        Self {
            config,
            client,
            healthy,
            shutdown_tx,
        }
    }

    /// Background health check loop.
    async fn health_check_loop(
        client: Arc<Client>,
        config: OcrConfig,
        healthy: Arc<AtomicBool>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let health_url = format!("{}/health", config.base_url);
        let health_timeout = config.health_timeout();

        // Perform initial health check
        Self::perform_health_check(&client, &health_url, health_timeout, &healthy).await;

        loop {
            tokio::select! {
                _ = sleep(config.health_interval()) => {
                    Self::perform_health_check(&client, &health_url, health_timeout, &healthy).await;
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::debug!("OCR health check loop shutting down");
                        return;
                    }
                }
            }
        }
    }

    /// Perform a single health check with custom timeout.
    async fn perform_health_check(
        client: &Client,
        url: &str,
        timeout: std::time::Duration,
        healthy: &AtomicBool,
    ) {
        // Use tokio::time::timeout to enforce health check timeout
        let result = tokio::time::timeout(timeout, client.get(url).send()).await;
        match result {
            Ok(Ok(resp)) if resp.status().is_success() => {
                // Consume response body to properly close the connection
                let _ = resp.bytes().await;
                if !healthy.load(Ordering::Relaxed) {
                    tracing::info!("OCR service is now healthy");
                }
                healthy.store(true, Ordering::Relaxed);
            }
            Ok(Ok(resp)) => {
                let status = resp.status();
                let _ = resp.bytes().await;
                if healthy.load(Ordering::Relaxed) {
                    tracing::warn!("OCR service returned non-success status: {}", status);
                }
                healthy.store(false, Ordering::Relaxed);
            }
            Ok(Err(e)) => {
                // Request failed
                let was_healthy = healthy.load(Ordering::Relaxed);
                healthy.store(false, Ordering::Relaxed);
                if was_healthy {
                    if e.is_connect() {
                        tracing::warn!("OCR health check: connection refused (service may be down)");
                    } else {
                        tracing::warn!("OCR health check failed: {}", e);
                    }
                }
            }
            Err(_) => {
                // Timeout
                let was_healthy = healthy.load(Ordering::Relaxed);
                healthy.store(false, Ordering::Relaxed);
                if was_healthy {
                    tracing::warn!("OCR health check: timeout");
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct OcrApiResponse {
    numerator: i32,
    denominator: i32,
    debug: OcrDebugInfo,
}

#[derive(Debug, Deserialize)]
struct OcrDebugInfo {
    raw_text: String,
    confidence: f64,
}

#[async_trait]
impl OcrClient for HttpOcrClient {
    fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    async fn recognize_usage_ratio(
        &self,
        image: &DynamicImage,
        roi: Option<&Roi>,
    ) -> anyhow::Result<UsageRatioResult> {
        if !self.is_healthy() {
            anyhow::bail!("OCR service is currently unavailable");
        }

        // Crop image if ROI is specified
        let target_image = if let Some(roi) = roi {
            image.crop_imm(
                roi.x as u32,
                roi.y as u32,
                roi.width as u32,
                roi.height as u32,
            )
        } else {
            image.clone()
        };

        // Encode to PNG
        let mut png_bytes = Vec::new();
        target_image.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )?;

        // Build request URL
        let url = format!("{}/v1/ratios/usage", self.config.base_url);

        // Send request
        let response: reqwest::Response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(png_bytes)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("No ratio found in image");
        }

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            anyhow::bail!("OCR request failed with status {}: {}", status, body);
        }

        let api_response: OcrApiResponse = response.json().await?;

        Ok(UsageRatioResult {
            numerator: api_response.numerator,
            denominator: api_response.denominator,
            raw_text: api_response.debug.raw_text,
            confidence: api_response.debug.confidence,
        })
    }

    fn close(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Ensure background health check thread is stopped when client is dropped
impl Drop for HttpOcrClient {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        tracing::debug!("HttpOcrClient dropped, health check loop signaled to stop");
    }
}
