use super::driver::BrowserDriver;
use crate::domain::model::Cookie;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
};
use chromiumoxide::page::Page;
use futures::StreamExt;
use image::DynamicImage;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

/// Chromium browser driver using chromiumoxide
pub struct ChromiumDriver {
    session_id: String,
    browser: RwLock<Option<Browser>>,
    page: RwLock<Option<Arc<Mutex<Page>>>>,
    handler_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    frame_tx: mpsc::UnboundedSender<String>,
    screenshot_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    user_data_dir: PathBuf,
    viewport_width: u32,
    viewport_height: u32,
}

impl ChromiumDriver {
    /// Create a new ChromiumDriver with a unique user data directory per session
    pub fn new(session_id: &str, frame_tx: mpsc::UnboundedSender<String>) -> Self {
        // Create unique user data directory for this session to avoid SingletonLock conflicts
        let user_data_dir = std::env::temp_dir()
            .join("wardenly-browsers")
            .join(session_id);
        
        Self {
            session_id: session_id.to_string(),
            browser: RwLock::new(None),
            page: RwLock::new(None),
            handler_handle: RwLock::new(None),
            frame_tx,
            screenshot_handle: RwLock::new(None),
            user_data_dir,
            viewport_width: 1080,
            viewport_height: 720,
        }
    }

    async fn page(&self) -> Result<Arc<Mutex<Page>>> {
        self.page
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow!("Browser not started"))
    }
    
    /// Cleanup user data directory after browser stops
    fn cleanup_user_data_dir(&self) {
        if self.user_data_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.user_data_dir) {
                tracing::warn!(
                    "Failed to cleanup user data dir {:?}: {}",
                    self.user_data_dir,
                    e
                );
            }
        }
    }
}

#[async_trait]
impl BrowserDriver for ChromiumDriver {
    async fn start(&self) -> Result<()> {
        // Ensure user data directory exists
        if let Err(e) = std::fs::create_dir_all(&self.user_data_dir) {
            return Err(anyhow!("Failed to create user data dir: {}", e));
        }
        
        tracing::info!(
            "Starting browser for session {} with user data dir: {:?}",
            self.session_id,
            self.user_data_dir
        );
        
        let config = BrowserConfig::builder()
            .window_size(self.viewport_width, self.viewport_height + 120)
            .viewport(chromiumoxide::handler::viewport::Viewport {
                width: self.viewport_width,
                height: self.viewport_height,
                device_scale_factor: None,
                emulating_mobile: false,
                is_landscape: false,
                has_touch: false,
            })
            // Use unique user data directory per session to avoid SingletonLock conflicts
            .user_data_dir(&self.user_data_dir)
            // Enable headless mode for better performance (no visible window)
            .arg("--headless=new")
            // Enable GPU acceleration for better rendering performance
            // (GPU is enabled by default, we just don't disable it)
            // Disable infobars
            .arg("--disable-infobars")
            // Mute audio
            .arg("--mute-audio")
            // Disable unnecessary features for headless
            .arg("--hide-scrollbars")
            .arg("--disable-web-security")
            .build()
            .map_err(|e| anyhow!("Failed to build browser config: {}", e))?;

        let (browser, mut handler) = Browser::launch(config).await?;

        // Spawn handler task
        let handler_handle = tokio::spawn(async move {
            while let Some(_event) = handler.next().await {
                // Events are handled internally by chromiumoxide
            }
        });

        let page = browser.new_page("about:blank").await?;

        *self.browser.write().await = Some(browser);
        *self.page.write().await = Some(Arc::new(Mutex::new(page)));
        *self.handler_handle.write().await = Some(handler_handle);

        tracing::info!("Browser started successfully for session {}", self.session_id);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping browser for session {}", self.session_id);
        
        // Stop screenshot task first
        if let Some(handle) = self.screenshot_handle.write().await.take() {
            handle.abort();
        }

        // Close browser
        if let Some(mut browser) = self.browser.write().await.take() {
            let _ = browser.close().await;
        }

        // Abort handler
        if let Some(handle) = self.handler_handle.write().await.take() {
            handle.abort();
        }

        *self.page.write().await = None;
        
        // Cleanup user data directory
        self.cleanup_user_data_dir();

        tracing::info!("Browser stopped for session {}", self.session_id);
        Ok(())
    }

    async fn navigate(&self, url: &str) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;
        page.goto(url).await?;
        tracing::debug!("Navigated to {}", url);
        Ok(())
    }

    async fn click(&self, x: f64, y: f64) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;

        // Move mouse
        let move_params = DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, x, y);
        page.execute(move_params).await?;

        // Mouse down
        let mut down_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MousePressed, x, y);
        down_params.button = Some(MouseButton::Left);
        down_params.click_count = Some(1);
        page.execute(down_params).await?;

        // Mouse up
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, x, y);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        page.execute(up_params).await?;

        tracing::trace!("Clicked at ({}, {})", x, y);
        Ok(())
    }

    async fn drag(&self, from: (f64, f64), to: (f64, f64)) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;

        // Move to start position
        let move_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, from.0, from.1);
        page.execute(move_params).await?;

        // Mouse down at start
        let mut down_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MousePressed, from.0, from.1);
        down_params.button = Some(MouseButton::Left);
        down_params.click_count = Some(1);
        page.execute(down_params).await?;

        // Move to end position
        let move_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, to.0, to.1);
        page.execute(move_params).await?;

        // Mouse up at end
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, to.0, to.1);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        page.execute(up_params).await?;

        tracing::trace!("Dragged from {:?} to {:?}", from, to);
        Ok(())
    }

    async fn start_screencast(&self) -> Result<()> {
        // Idempotent: if already running, do nothing
        {
            let handle = self.screenshot_handle.read().await;
            if handle.is_some() {
                tracing::debug!("Screencast already running, skipping start");
                return Ok(());
            }
        }
        
        let page = self.page().await?;
        let frame_tx = self.frame_tx.clone();

        // Use periodic screenshots as screencast (~3 FPS with JPEG for better performance)
        // 3 FPS (333ms) is sufficient for game automation and reduces CPU load
        let handle = tokio::spawn(async move {
            use chromiumoxide::cdp::browser_protocol::page::{
                CaptureScreenshotFormat, CaptureScreenshotParams,
            };
            
            loop {
                // 3 FPS interval (333ms) - good balance between responsiveness and performance
                tokio::time::sleep(tokio::time::Duration::from_millis(333)).await;

                let page_guard = page.lock().await;
                // Use JPEG format with quality 80 for much better performance than PNG
                let params = CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Jpeg)
                    .quality(80)
                    .build();
                    
                match page_guard.screenshot(params).await {
                    Ok(data) => {
                        use base64::Engine;
                        let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);
                        // Non-blocking send - if channel is full, the frame is dropped
                        // This prevents frame backlog when frontend can't keep up
                        if frame_tx.send(base64_data).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::trace!("Screenshot failed: {}", e);
                    }
                }
                drop(page_guard);
            }
        });

        *self.screenshot_handle.write().await = Some(handle);

        tracing::info!("Screencast started (JPEG @ 3 FPS)");
        Ok(())
    }

    async fn stop_screencast(&self) -> Result<()> {
        if let Some(handle) = self.screenshot_handle.write().await.take() {
            handle.abort();
        }
        tracing::info!("Screencast stopped");
        Ok(())
    }

    async fn set_cookies(&self, cookies: &[Cookie]) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;

        for cookie in cookies {
            let mut params = chromiumoxide::cdp::browser_protocol::network::SetCookieParams::new(
                cookie.name.clone(),
                cookie.value.clone(),
            );
            params.domain = Some(cookie.domain.clone());
            params.path = Some(cookie.path.clone());
            params.secure = Some(cookie.secure);
            params.http_only = Some(cookie.http_only);

            page.execute(params).await?;
        }

        tracing::debug!("Set {} cookies", cookies.len());
        Ok(())
    }

    async fn get_cookies(&self) -> Result<Vec<Cookie>> {
        let page = self.page().await?;
        let page = page.lock().await;

        let result = page
            .execute(
                chromiumoxide::cdp::browser_protocol::network::GetCookiesParams::default(),
            )
            .await?;

        let cookies: Vec<Cookie> = result
            .result
            .cookies
            .into_iter()
            .map(|c| Cookie {
                name: c.name,
                value: c.value,
                domain: c.domain,
                path: c.path,
                http_only: c.http_only,
                secure: c.secure,
            })
            .collect();

        Ok(cookies)
    }

    async fn evaluate(&self, script: &str) -> Result<String> {
        let page = self.page().await?;
        let page = page.lock().await;

        let result = page.evaluate(script).await?;
        let value: serde_json::Value = result.into_value()?;
        Ok(value.to_string())
    }

    async fn capture_screen(&self) -> Result<DynamicImage> {
        let page = self.page().await?;
        let page = page.lock().await;

        let data = page
            .screenshot(
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::builder()
                    .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
                    .build(),
            )
            .await?;

        let img = image::load_from_memory(&data)?;
        Ok(img)
    }

    async fn input_text(&self, selector: &str, text: &str) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;

        // Find element and type text
        let element = page.find_element(selector).await?;
        element.click().await?;
        element.type_str(text).await?;

        tracing::debug!("Input text into {}", selector);
        Ok(())
    }

    async fn click_element(&self, selector: &str) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;

        let element = page.find_element(selector).await?;
        element.click().await?;

        tracing::debug!("Clicked element {}", selector);
        Ok(())
    }
    
    async fn wait_visible(&self, selector: &str, timeout: std::time::Duration) -> Result<()> {
        let page = self.page().await?;
        let start = std::time::Instant::now();
        
        loop {
            if start.elapsed() > timeout {
                return Err(anyhow!("Timeout waiting for element: {}", selector));
            }
            
            let page_guard = page.lock().await;
            match page_guard.find_element(selector).await {
                Ok(_) => {
                    tracing::debug!("Element {} is visible", selector);
                    return Ok(());
                }
                Err(_) => {
                    drop(page_guard);
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
    }
    
    async fn login_with_password(
        &self,
        url: &str,
        username: &str,
        password: &str,
        timeout: std::time::Duration,
    ) -> Result<()> {
        tracing::info!("Starting login with password for URL: {}", url);
        
        // Navigate to game URL
        self.navigate(url).await?;
        
        // Wait for username field to be visible
        self.wait_visible("#username", timeout).await?;
        
        // Input username
        self.input_text("#username", username).await?;
        
        // Input password (use #userpwd as per wardenly-go)
        self.input_text("#userpwd", password).await?;
        
        // Click login button (selector from wardenly-go)
        self.click_element("#form1 > div.r06 > div.login_box3 > p > input").await?;
        
        // Wait for game iframe to appear (indicates successful login)
        self.wait_visible("#S_Iframe", timeout).await?;
        
        tracing::info!("Login with password completed successfully");
        Ok(())
    }
    
    async fn login_with_cookies(
        &self,
        url: &str,
        cookies: &[Cookie],
        timeout: std::time::Duration,
    ) -> Result<()> {
        tracing::info!("Starting login with cookies for URL: {}", url);
        
        // Set cookies first
        self.set_cookies(cookies).await?;
        
        // Navigate to game URL
        self.navigate(url).await?;
        
        // Wait for game iframe to appear (indicates cookies are valid)
        self.wait_visible("#S_Iframe", timeout).await?;
        
        tracing::info!("Login with cookies completed successfully");
        Ok(())
    }
    
    async fn refresh(&self) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;
        
        page.reload().await?;
        
        tracing::info!("Page refreshed");
        Ok(())
    }
}
