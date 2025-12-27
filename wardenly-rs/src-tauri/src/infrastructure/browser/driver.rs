use async_trait::async_trait;

/// Browser driver trait for abstracting browser automation
#[allow(dead_code)]
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    /// Start the browser instance
    async fn start(&mut self) -> anyhow::Result<()>;

    /// Stop the browser instance
    async fn stop(&mut self) -> anyhow::Result<()>;

    /// Navigate to a URL
    async fn navigate(&self, url: &str) -> anyhow::Result<()>;

    /// Click at coordinates
    async fn click(&self, x: f64, y: f64) -> anyhow::Result<()>;

    /// Drag from one point to another
    async fn drag(&self, from: (f64, f64), to: (f64, f64)) -> anyhow::Result<()>;

    /// Start screencast streaming
    async fn start_screencast(&self) -> anyhow::Result<()>;

    /// Stop screencast streaming
    async fn stop_screencast(&self) -> anyhow::Result<()>;

    /// Set cookies for the browser
    async fn set_cookies(&self, cookies: &str) -> anyhow::Result<()>;

    /// Get cookies from the browser
    async fn get_cookies(&self) -> anyhow::Result<String>;

    /// Execute JavaScript and return result
    async fn evaluate(&self, script: &str) -> anyhow::Result<String>;
}

