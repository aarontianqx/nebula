use super::driver::BrowserDriver;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
};
use chromiumoxide::page::Page;
use futures::StreamExt;
use image::DynamicImage;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

/// Chromium browser driver using chromiumoxide
pub struct ChromiumDriver {
    browser: RwLock<Option<Browser>>,
    page: RwLock<Option<Arc<Mutex<Page>>>>,
    handler_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    frame_tx: mpsc::UnboundedSender<String>,
    screenshot_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
    viewport_width: u32,
    viewport_height: u32,
}

impl ChromiumDriver {
    pub fn new(frame_tx: mpsc::UnboundedSender<String>) -> Self {
        Self {
            browser: RwLock::new(None),
            page: RwLock::new(None),
            handler_handle: RwLock::new(None),
            frame_tx,
            screenshot_handle: RwLock::new(None),
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
}

#[async_trait]
impl BrowserDriver for ChromiumDriver {
    async fn start(&self) -> Result<()> {
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

        tracing::info!("Browser started successfully");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
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

        tracing::info!("Browser stopped");
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
        let page = self.page().await?;
        let frame_tx = self.frame_tx.clone();

        // Use periodic screenshots as screencast
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                let page_guard = page.lock().await;
                match page_guard
                    .screenshot(
                        chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::default(
                        ),
                    )
                    .await
                {
                    Ok(data) => {
                        use base64::Engine;
                        let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);
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

        tracing::info!("Screencast started (using periodic screenshots)");
        Ok(())
    }

    async fn stop_screencast(&self) -> Result<()> {
        if let Some(handle) = self.screenshot_handle.write().await.take() {
            handle.abort();
        }
        tracing::info!("Screencast stopped");
        Ok(())
    }

    async fn set_cookies(&self, cookies_json: &str) -> Result<()> {
        let page = self.page().await?;
        let page = page.lock().await;

        let cookies: Vec<chromiumoxide::cdp::browser_protocol::network::CookieParam> =
            serde_json::from_str(cookies_json)?;

        for cookie in cookies {
            let mut params = chromiumoxide::cdp::browser_protocol::network::SetCookieParams::new(
                cookie.name.clone(),
                cookie.value.clone(),
            );
            params.domain = cookie.domain.clone();

            page.execute(params).await?;
        }

        Ok(())
    }

    async fn get_cookies(&self) -> Result<String> {
        let page = self.page().await?;
        let page = page.lock().await;

        let cookies = page
            .execute(
                chromiumoxide::cdp::browser_protocol::network::GetCookiesParams::default(),
            )
            .await?;

        Ok(serde_json::to_string(&cookies.result.cookies)?)
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
}
