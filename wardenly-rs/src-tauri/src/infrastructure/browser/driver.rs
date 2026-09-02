use async_trait::async_trait;
use image::DynamicImage;
use std::time::Duration;
use tokio::sync::mpsc;

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

    /// Execute JavaScript in the top-level page and return the JSON-serialized result.
    ///
    /// Note: this only reaches the top-level page context, never cross-origin iframes;
    /// callers needing the game JS context must navigate the tab directly to the game URL.
    async fn evaluate(&self, script: &str) -> anyhow::Result<String>;

    /// Install the page bridge: register a CDP `Runtime.addBinding` push channel
    /// (page → host) plus an init script evaluated before page scripts on every
    /// new document. Returns a receiver yielding the string payloads the page
    /// passes to the binding.
    async fn install_page_bridge(
        &self,
        binding_name: &str,
        init_script: &str,
    ) -> anyhow::Result<mpsc::Receiver<String>>;

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
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> anyhow::Result<()>;

    /// Refresh/reload the current page
    async fn refresh(&self) -> anyhow::Result<()>;

    /// Insert text into the currently focused element.
    /// Uses CDP Input.insertText for full Unicode/CJK support.
    async fn insert_text(&self, text: &str) -> anyhow::Result<()>;
}
