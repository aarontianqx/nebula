use async_trait::async_trait;
use image::DynamicImage;
use std::time::Duration;

use crate::domain::model::Cookie;

/// Point for browser coordinate operations.
/// Separate from domain::model::Point to maintain layer separation.
#[derive(Debug, Clone, Copy)]
pub struct BrowserPoint {
    pub x: f64,
    pub y: f64,
}

impl BrowserPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Browser driver trait for abstracting browser automation
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    /// Start the browser instance
    async fn start(&self) -> anyhow::Result<()>;

    /// Stop the browser instance
    async fn stop(&self) -> anyhow::Result<()>;

    /// Navigate to a URL
    async fn navigate(&self, url: &str) -> anyhow::Result<()>;

    /// Click at coordinates
    async fn click(&self, x: f64, y: f64) -> anyhow::Result<()>;

    /// Drag from one point to another with smooth interpolation (10 steps, 60fps timing)
    async fn drag(&self, from: (f64, f64), to: (f64, f64)) -> anyhow::Result<()>;

    /// Drag along a path of points with frame-based timing.
    /// Requires at least 2 points. Each segment uses 60fps timing for smooth movement.
    async fn drag_path(&self, points: &[BrowserPoint]) -> anyhow::Result<()>;

    /// Start screencast streaming
    async fn start_screencast(&self) -> anyhow::Result<()>;

    /// Stop screencast streaming
    async fn stop_screencast(&self) -> anyhow::Result<()>;

    /// Set cookies for the browser
    async fn set_cookies(&self, cookies: &[Cookie]) -> anyhow::Result<()>;

    /// Get cookies from the browser
    async fn get_cookies(&self) -> anyhow::Result<Vec<Cookie>>;

    /// Execute JavaScript and return result
    async fn evaluate(&self, script: &str) -> anyhow::Result<String>;

    /// Capture the current screen as an image
    async fn capture_screen(&self) -> anyhow::Result<DynamicImage>;

    /// Input text into a form field by selector
    async fn input_text(&self, selector: &str, text: &str) -> anyhow::Result<()>;

    /// Click an element by selector
    async fn click_element(&self, selector: &str) -> anyhow::Result<()>;
    
    /// Wait for an element to be visible
    async fn wait_visible(&self, selector: &str, timeout: Duration) -> anyhow::Result<()>;
    
    /// Perform complete login with username/password
    /// This executes all steps atomically like wardenly-go
    async fn login_with_password(
        &self,
        url: &str,
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> anyhow::Result<()>;
    
    /// Perform complete login with cookies
    /// This sets cookies and navigates to the game URL
    async fn login_with_cookies(
        &self,
        url: &str,
        cookies: &[Cookie],
        timeout: Duration,
    ) -> anyhow::Result<()>;
    
    /// Refresh/reload the current page
    async fn refresh(&self) -> anyhow::Result<()>;
}

