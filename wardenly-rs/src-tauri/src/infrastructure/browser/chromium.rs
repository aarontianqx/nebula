use super::driver::{BrowserDriver, BrowserPoint};
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
    account_id: String,
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
    /// Create a new ChromiumDriver with a persistent user data directory per account.
    /// Profile data (cache, cookies, localStorage) is preserved across sessions.
    pub fn new(session_id: &str, account_id: &str, frame_tx: mpsc::UnboundedSender<String>) -> Self {
        // Use centralized path utility for consistency with delete_profile()
        use crate::infrastructure::config::paths;
        let user_data_dir = paths::profile_dir(account_id);
        
        Self {
            session_id: session_id.to_string(),
            account_id: account_id.to_string(),
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

    /// Clean stale lockfiles left by crashed browser instances.
    /// Chrome creates "SingletonLock" and "SingletonSocket" files that prevent
    /// multiple processes from using the same profile directory.
    fn clean_stale_lockfiles(&self) {
        let lockfile_names = ["SingletonLock", "SingletonSocket", "SingletonCookie"];
        for name in lockfile_names {
            let lockfile = self.user_data_dir.join(name);
            if lockfile.exists() {
                if let Err(e) = std::fs::remove_file(&lockfile) {
                    tracing::warn!("Failed to remove stale lockfile {:?}: {}", lockfile, e);
                } else {
                    tracing::debug!("Removed stale lockfile: {:?}", lockfile);
                }
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
        
        // Clean stale lockfiles from previous crashed sessions
        self.clean_stale_lockfiles();
        
        tracing::info!(
            "Starting browser for session {} (account {}) with profile: {:?}",
            self.session_id,
            self.account_id,
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
        
        // NOTE: Profile directory is NOT cleaned up to preserve cache for faster startup next time.
        // See docs/roadmap/BROWSER_PERSISTENCE_RFC.md for rationale.

        tracing::info!("Browser stopped for session {} (profile preserved at {:?})", self.session_id, self.user_data_dir);
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
        // Frame interval for 60 FPS (~16.67ms)
        const FRAME_INTERVAL_NS: u64 = 16_666_667;
        const INTERPOLATION_STEPS: usize = 10;

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

        // Interpolate movement in steps for smooth, realistic dragging
        let delta_x = (to.0 - from.0) / INTERPOLATION_STEPS as f64;
        let delta_y = (to.1 - from.1) / INTERPOLATION_STEPS as f64;

        for i in 1..=INTERPOLATION_STEPS {
            let x = from.0 + delta_x * i as f64;
            let y = from.1 + delta_y * i as f64;
            
            let move_params =
                DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, x, y);
            page.execute(move_params).await?;

            // Frame-based timing for smooth movement
            tokio::time::sleep(std::time::Duration::from_nanos(FRAME_INTERVAL_NS)).await;
        }

        // Mouse up at end
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, to.0, to.1);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        page.execute(up_params).await?;

        tracing::trace!("Dragged from {:?} to {:?} with {} steps", from, to, INTERPOLATION_STEPS);
        Ok(())
    }

    async fn drag_path(&self, points: &[BrowserPoint]) -> Result<()> {
        if points.len() < 2 {
            return Err(anyhow!("drag_path requires at least 2 points"));
        }

        // Frame interval for 60 FPS (~16.67ms)
        const FRAME_INTERVAL_NS: u64 = 16_666_667;

        let page = self.page().await?;
        let page = page.lock().await;

        let start = &points[0];

        // Move to start position
        let move_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, start.x, start.y);
        page.execute(move_params).await?;

        // Mouse down at start
        let mut down_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MousePressed, start.x, start.y);
        down_params.button = Some(MouseButton::Left);
        down_params.click_count = Some(1);
        page.execute(down_params).await?;

        // Move through all intermediate points with frame-based timing
        for point in points.iter().skip(1) {
            let move_params =
                DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, point.x, point.y);
            page.execute(move_params).await?;

            // Frame delay between moves for smooth, realistic dragging
            tokio::time::sleep(std::time::Duration::from_nanos(FRAME_INTERVAL_NS)).await;
        }

        // Mouse up at end position
        let end = &points[points.len() - 1];
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, end.x, end.y);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        page.execute(up_params).await?;

        tracing::trace!("Dragged path with {} points", points.len());
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
        username: &str,
        password: &str,
        timeout: std::time::Duration,
    ) -> Result<()> {
        tracing::info!("Starting password login flow");
        
        // Wait for username field to be visible
        self.wait_visible("#username", timeout).await?;
        
        // Input username
        self.input_text("#username", username).await?;
        
        // Input password (use #userpwd as per wardenly-go)
        self.input_text("#userpwd", password).await?;
        
        // Click login button (selector from wardenly-go)
        self.click_element("#form1 > div.r06 > div.login_box3 > p > input").await?;
        
        tracing::info!("Password login form submitted");
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
