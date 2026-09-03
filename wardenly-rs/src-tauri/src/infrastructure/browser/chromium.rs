use super::driver::{BrowserDriver, BrowserPoint};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetFocusEmulationEnabledParams;
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, InsertTextParams, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, CaptureScreenshotParams, EventJavascriptDialogOpening,
    HandleJavaScriptDialogParams,
};
use chromiumoxide::cdp::browser_protocol::target::{CloseTargetParams, EventTargetCreated};
use chromiumoxide::cdp::js_protocol::runtime::{AddBindingParams, EventBindingCalled};
use chromiumoxide::page::Page;
use futures::StreamExt;
use image::DynamicImage;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// Hard ceiling for any single interactive CDP call (click, screenshot, evaluate...).
/// Without this, a CDP command issued against a renderer that is blocked (e.g. by a
/// JavaScript dialog) or a backgrounded tab that stopped compositing would wait forever,
/// permanently wedging the session. The timeout guarantees every call returns.
const CDP_TIMEOUT: Duration = Duration::from_secs(5);

/// Longer ceiling for navigation-class calls which legitimately take a while.
const NAV_TIMEOUT: Duration = Duration::from_secs(30);

/// Ceiling for browser close/kill during shutdown so stop() can never hang.
const BROWSER_CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

/// Run a CDP future under a hard timeout, normalizing both the inner error and the
/// elapsed timeout into `anyhow::Error`. This is the single most important guard against
/// the "stuck forever" failure mode: a timed-out call returns an error and, crucially,
/// drops the future so no lock or resource is held indefinitely.
async fn with_timeout<T, E, F>(dur: Duration, op: &str, fut: F) -> Result<T>
where
    F: Future<Output = std::result::Result<T, E>>,
    E: std::fmt::Display,
{
    match tokio::time::timeout(dur, fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(anyhow!("{} failed: {}", op, e)),
        Err(_) => Err(anyhow!("{} timed out after {:?}", op, dur)),
    }
}

/// Humanization: random coordinate offset within ±2px (hand tremor), so
/// synthetic clicks don't land on exact integers every time. Button hit areas
/// are far larger than this, so it never changes click semantics.
fn jitter_point(x: f64, y: f64) -> (f64, f64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (x + rng.gen_range(-2.0..=2.0), y + rng.gen_range(-2.0..=2.0))
}

/// Humanization: randomized mouse-down → up hold time (30–90ms), so presses
/// don't fire at zero duration at machine cadence.
fn human_hold() -> Duration {
    use rand::Rng;
    Duration::from_millis(rand::thread_rng().gen_range(30..=90))
}

/// Chromium browser driver using chromiumoxide.
///
/// Resilience design (see specs/proposals/popup-resilience.md):
/// - `Page` is cheap to clone (internally `Arc`) and is shared lock-free across the
///   screencast, command, and script tasks so a slow/stuck screenshot can never block input.
/// - `input_lock` only serializes multi-step input *sequences* (move/down/up), keeping
///   pointer ordering correct without coupling input latency to screenshot latency.
/// - Background listeners auto-dismiss JS dialogs and auto-close stray popup targets.
pub struct ChromiumDriver {
    session_id: String,
    account_id: String,
    /// Shared so the popup-closing task can issue `Target.closeTarget` on the browser session.
    browser: Arc<RwLock<Option<Browser>>>,
    /// `Page` is `Clone` (Arc inner); no `Mutex` needed and none wanted (a stuck screenshot
    /// must not be able to block clicks).
    page: RwLock<Option<Page>>,
    /// Serializes only input event *sequences* to preserve pointer ordering.
    input_lock: Mutex<()>,
    handler_handle: RwLock<Option<JoinHandle<()>>>,
    /// Auto-dismisses `javascriptDialogOpening` (alert/confirm/prompt/beforeunload).
    dialog_handle: RwLock<Option<JoinHandle<()>>>,
    /// Auto-closes popup targets opened by our page (window.open / target=_blank).
    target_handle: RwLock<Option<JoinHandle<()>>>,
    frame_tx: mpsc::Sender<String>,
    screenshot_handle: RwLock<Option<JoinHandle<()>>>,
    user_data_dir: PathBuf,
    viewport_width: u32,
    viewport_height: u32,
}

impl ChromiumDriver {
    /// Create a new ChromiumDriver with a persistent user data directory per account.
    /// Profile data (cache, cookies, localStorage) is preserved across sessions.
    pub fn new(session_id: &str, account_id: &str, frame_tx: mpsc::Sender<String>) -> Self {
        // Use centralized path utility for consistency with delete_profile()
        use crate::infrastructure::config::paths;
        let user_data_dir = paths::profile_dir(account_id);

        Self {
            session_id: session_id.to_string(),
            account_id: account_id.to_string(),
            browser: Arc::new(RwLock::new(None)),
            page: RwLock::new(None),
            input_lock: Mutex::new(()),
            handler_handle: RwLock::new(None),
            dialog_handle: RwLock::new(None),
            target_handle: RwLock::new(None),
            frame_tx,
            screenshot_handle: RwLock::new(None),
            user_data_dir,
            viewport_width: 1080,
            viewport_height: 720,
        }
    }

    /// Clone the page handle. Cheap (`Arc` bump) and intentionally lock-free for callers.
    async fn page(&self) -> Result<Page> {
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

    /// Spawn the background task that auto-dismisses JavaScript dialogs.
    ///
    /// A `beforeunload`/`alert`/`confirm` opened by a mis-click would otherwise stall the
    /// renderer (CDP: "calling alert while Page domain is engaged will stall the page
    /// execution"), freezing screenshots and clicks. We dismiss (accept=false) so the page
    /// stays put and never blocks.
    fn spawn_dialog_dismisser(
        &self,
        page: Page,
        dialogs: chromiumoxide::listeners::EventStream<EventJavascriptDialogOpening>,
    ) -> JoinHandle<()> {
        let session_id = self.session_id.clone();
        tokio::spawn(async move {
            let mut dialogs = dialogs;
            while let Some(ev) = dialogs.next().await {
                tracing::warn!(
                    session_id = %session_id,
                    dialog_type = ?ev.r#type,
                    "Auto-dismissing JavaScript dialog: {}",
                    ev.message
                );
                // accept=false => dismiss; for beforeunload this means "stay on page".
                let _ = with_timeout(
                    CDP_TIMEOUT,
                    "handleJavaScriptDialog",
                    page.execute(HandleJavaScriptDialogParams::new(false)),
                )
                .await;
            }
        })
    }

    /// Spawn the background task that auto-closes popup targets opened by our page.
    ///
    /// Defense-in-depth behind `--block-new-web-contents`: should any new tab/window still
    /// appear (e.g. `target=_blank`), close it immediately so it can never steal foreground
    /// and background-throttle (and thus freeze the screenshots of) our game page.
    fn spawn_popup_closer(
        &self,
        own_target: chromiumoxide::cdp::browser_protocol::target::TargetId,
        targets: chromiumoxide::listeners::EventStream<EventTargetCreated>,
    ) -> JoinHandle<()> {
        let session_id = self.session_id.clone();
        let browser = self.browser.clone();
        tokio::spawn(async move {
            let mut targets = targets;
            while let Some(ev) = targets.next().await {
                let info = &ev.target_info;
                let is_popup = matches!(info.r#type.as_str(), "page" | "tab")
                    && info.target_id != own_target
                    && info.opener_id.as_ref() == Some(&own_target);
                if !is_popup {
                    continue;
                }
                tracing::warn!(
                    session_id = %session_id,
                    "Auto-closing popup target (url={})",
                    info.url
                );
                if let Some(b) = browser.read().await.as_ref() {
                    let _ = with_timeout(
                        CDP_TIMEOUT,
                        "closeTarget",
                        b.execute(CloseTargetParams::new(info.target_id.clone())),
                    )
                    .await;
                }
            }
        })
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
            // CRITICAL: block all pop-ups and window.open. A mis-clicked in-game popup can
            // otherwise open a new tab, which backgrounds (and stops compositing) our game
            // tab, making Page.captureScreenshot hang forever. This stops the problem at
            // the source. See specs/proposals/popup-resilience.md.
            .arg("--block-new-web-contents")
            // Enable GPU acceleration for better rendering performance
            // (GPU is enabled by default, we just don't disable it)
            // Disable infobars
            .arg("--disable-infobars")
            // Mute audio
            .arg("--mute-audio")
            // Disable unnecessary features for headless
            .arg("--hide-scrollbars")
            .arg("--disable-web-security")
            // Keep http pages on http: Chrome's HTTPS-First upgrade would force the game
            // page to https, where its plaintext ws:// connection is blocked as mixed
            // content and the game silently stalls on the loading screen.
            .arg("--disable-features=HttpsUpgrades")
            .build()
            .map_err(|e| anyhow!("Failed to build browser config: {}", e))?;

        let (browser, mut handler) = Browser::launch(config).await?;

        // Spawn handler task - drives the CDP connection IO and routes events to listeners.
        let handler_handle = tokio::spawn(async move {
            while let Some(_event) = handler.next().await {
                // Events are routed internally by chromiumoxide to per-page/browser listeners.
            }
        });

        let page = browser.new_page("about:blank").await?;

        // Keep the page compositing frames even if it is ever considered occluded/backgrounded,
        // so screenshots don't hang on a frame that never arrives (Playwright uses the same trick).
        if let Err(e) = with_timeout(
            CDP_TIMEOUT,
            "setFocusEmulationEnabled",
            page.execute(SetFocusEmulationEnabledParams::new(true)),
        )
        .await
        {
            tracing::warn!(
                "Focus emulation not enabled for session {}: {}",
                self.session_id,
                e
            );
        }

        // Subscribe to events BEFORE moving the browser into shared storage.
        let dialog_stream = page
            .event_listener::<EventJavascriptDialogOpening>()
            .await
            .map_err(|e| anyhow!("Failed to subscribe dialog events: {}", e))?;
        let target_stream = browser
            .event_listener::<EventTargetCreated>()
            .await
            .map_err(|e| anyhow!("Failed to subscribe target events: {}", e))?;
        let own_target = page.target_id().clone();

        // Publish the browser so the popup-closer can reach the browser session.
        *self.browser.write().await = Some(browser);

        let dialog_handle = self.spawn_dialog_dismisser(page.clone(), dialog_stream);
        let target_handle = self.spawn_popup_closer(own_target, target_stream);

        *self.page.write().await = Some(page);
        *self.handler_handle.write().await = Some(handler_handle);
        *self.dialog_handle.write().await = Some(dialog_handle);
        *self.target_handle.write().await = Some(target_handle);

        // Log the actual browser binary version — CDP event shapes differ
        // across Chrome versions, which matters when diagnosing parse failures
        // on other machines.
        if let Some(b) = self.browser.read().await.as_ref() {
            match b.version().await {
                Ok(v) => tracing::info!(
                    "Browser started successfully for session {} ({})",
                    self.session_id,
                    v.product
                ),
                Err(e) => {
                    tracing::info!(
                        "Browser started successfully for session {}",
                        self.session_id
                    );
                    tracing::debug!("Could not query browser version: {}", e);
                }
            }
        }
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping browser for session {}", self.session_id);

        // Stop background tasks first so nothing keeps touching the page during teardown.
        if let Some(handle) = self.screenshot_handle.write().await.take() {
            handle.abort();
            let _ = handle.await;
        }
        if let Some(handle) = self.dialog_handle.write().await.take() {
            handle.abort();
        }
        if let Some(handle) = self.target_handle.write().await.take() {
            handle.abort();
        }

        // Clear page handle before closing browser to release references quickly.
        *self.page.write().await = None;

        // Close browser process and CDP resources. Both steps are time-boxed so a wedged CDP
        // connection can never make shutdown hang. `kill()` signals the OS child directly
        // (independent of the CDP handler) and reaps it, guaranteeing no orphaned Chrome.
        if let Some(mut browser) = self.browser.write().await.take() {
            if tokio::time::timeout(BROWSER_CLOSE_TIMEOUT, browser.close())
                .await
                .is_err()
            {
                tracing::warn!(
                    "Graceful browser close timed out for session {}, forcing kill",
                    self.session_id
                );
            }
            // Force kill + reap regardless of how close() went (no-op if already exited).
            let _ = tokio::time::timeout(BROWSER_CLOSE_TIMEOUT, browser.kill()).await;
            drop(browser);
        }

        // Abort handler and wait briefly for it to settle to avoid orphaned runtime tasks.
        if let Some(handle) = self.handler_handle.write().await.take() {
            handle.abort();
            if tokio::time::timeout(Duration::from_secs(1), handle)
                .await
                .is_err()
            {
                tracing::warn!(
                    "Timed out waiting browser handler to stop for session {}",
                    self.session_id
                );
            }
        }

        // NOTE: Profile directory is NOT cleaned up to preserve cache for faster startup next time.
        // See docs/roadmap/BROWSER_PERSISTENCE_RFC.md for rationale.

        tracing::info!(
            "Browser stopped for session {} (profile preserved at {:?})",
            self.session_id,
            self.user_data_dir
        );
        Ok(())
    }

    async fn navigate(&self, url: &str) -> Result<()> {
        let page = self.page().await?;
        with_timeout(NAV_TIMEOUT, "navigate", async {
            page.goto(url).await.map(|_| ())
        })
        .await?;
        tracing::debug!("Navigated to {}", url);
        Ok(())
    }

    async fn click(&self, x: f64, y: f64) -> Result<()> {
        let page = self.page().await?;
        let _input = self.input_lock.lock().await;

        // Humanization: jitter the target and randomize the hold time.
        let (x, y) = jitter_point(x, y);

        // Move mouse
        let move_params = DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, x, y);
        with_timeout(CDP_TIMEOUT, "mouse move", page.execute(move_params)).await?;

        // Mouse down
        let mut down_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MousePressed, x, y);
        down_params.button = Some(MouseButton::Left);
        down_params.click_count = Some(1);
        with_timeout(CDP_TIMEOUT, "mouse down", page.execute(down_params)).await?;

        sleep(human_hold()).await;

        // Mouse up
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, x, y);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        with_timeout(CDP_TIMEOUT, "mouse up", page.execute(up_params)).await?;

        tracing::trace!("Clicked at ({}, {})", x, y);
        Ok(())
    }

    async fn drag(&self, from: (f64, f64), to: (f64, f64)) -> Result<()> {
        // Frame interval for 60 FPS (~16.67ms)
        const FRAME_INTERVAL_NS: u64 = 16_666_667;
        const INTERPOLATION_STEPS: usize = 10;

        let page = self.page().await?;
        let _input = self.input_lock.lock().await;

        // Humanization: jitter both endpoints.
        let from = jitter_point(from.0, from.1);
        let to = jitter_point(to.0, to.1);

        // Move to start position
        let move_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, from.0, from.1);
        with_timeout(CDP_TIMEOUT, "drag move", page.execute(move_params)).await?;

        // Mouse down at start
        let mut down_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MousePressed, from.0, from.1);
        down_params.button = Some(MouseButton::Left);
        down_params.click_count = Some(1);
        with_timeout(CDP_TIMEOUT, "drag down", page.execute(down_params)).await?;

        sleep(human_hold()).await;

        // Interpolate movement in steps for smooth, realistic dragging
        let delta_x = (to.0 - from.0) / INTERPOLATION_STEPS as f64;
        let delta_y = (to.1 - from.1) / INTERPOLATION_STEPS as f64;

        for i in 1..=INTERPOLATION_STEPS {
            let x = from.0 + delta_x * i as f64;
            let y = from.1 + delta_y * i as f64;

            let move_params =
                DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, x, y);
            with_timeout(CDP_TIMEOUT, "drag move", page.execute(move_params)).await?;

            // Frame-based timing for smooth movement
            sleep(Duration::from_nanos(FRAME_INTERVAL_NS)).await;
        }

        // Mouse up at end
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, to.0, to.1);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        with_timeout(CDP_TIMEOUT, "drag up", page.execute(up_params)).await?;

        tracing::trace!(
            "Dragged from {:?} to {:?} with {} steps",
            from,
            to,
            INTERPOLATION_STEPS
        );
        Ok(())
    }

    async fn drag_path(&self, points: &[BrowserPoint]) -> Result<()> {
        if points.len() < 2 {
            return Err(anyhow!("drag_path requires at least 2 points"));
        }

        // Frame interval for 60 FPS (~16.67ms)
        const FRAME_INTERVAL_NS: u64 = 16_666_667;

        let page = self.page().await?;
        let _input = self.input_lock.lock().await;

        let start = &points[0];
        let start_pos = jitter_point(start.x, start.y);

        // Move to start position
        let move_params = DispatchMouseEventParams::new(
            DispatchMouseEventType::MouseMoved,
            start_pos.0,
            start_pos.1,
        );
        with_timeout(CDP_TIMEOUT, "drag_path move", page.execute(move_params)).await?;

        // Mouse down at start
        let mut down_params = DispatchMouseEventParams::new(
            DispatchMouseEventType::MousePressed,
            start_pos.0,
            start_pos.1,
        );
        down_params.button = Some(MouseButton::Left);
        down_params.click_count = Some(1);
        with_timeout(CDP_TIMEOUT, "drag_path down", page.execute(down_params)).await?;

        sleep(human_hold()).await;

        // Move through all intermediate points with frame-based timing
        for point in points.iter().skip(1) {
            let move_params =
                DispatchMouseEventParams::new(DispatchMouseEventType::MouseMoved, point.x, point.y);
            with_timeout(CDP_TIMEOUT, "drag_path move", page.execute(move_params)).await?;

            // Frame delay between moves for smooth, realistic dragging
            sleep(Duration::from_nanos(FRAME_INTERVAL_NS)).await;
        }

        // Mouse up at end position
        let end = &points[points.len() - 1];
        let mut up_params =
            DispatchMouseEventParams::new(DispatchMouseEventType::MouseReleased, end.x, end.y);
        up_params.button = Some(MouseButton::Left);
        up_params.click_count = Some(1);
        with_timeout(CDP_TIMEOUT, "drag_path up", page.execute(up_params)).await?;

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

        // Clone the page handle; the loop runs lock-free so a stuck/slow capture can never
        // block clicks or other operations.
        let page = self.page().await?;
        let frame_tx = self.frame_tx.clone();

        // Use periodic screenshots as screencast (~3 FPS with JPEG for better performance).
        // Each capture is time-boxed: if the renderer is wedged we simply skip the frame and
        // retry, instead of hanging forever.
        let handle = tokio::spawn(async move {
            let mut dropped_frames: u64 = 0;

            loop {
                // 3 FPS interval (333ms) - good balance between responsiveness and performance
                sleep(Duration::from_millis(333)).await;

                // Use JPEG format with quality 80 for much better performance than PNG
                let params = CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Jpeg)
                    .quality(80)
                    .build();

                match with_timeout(
                    CDP_TIMEOUT,
                    "screencast screenshot",
                    page.screenshot(params),
                )
                .await
                {
                    Ok(data) => {
                        use base64::Engine;
                        let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);
                        // Non-blocking send; drop frame when queue is full to avoid backlog growth.
                        match frame_tx.try_send(base64_data) {
                            Ok(()) => {}
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                dropped_frames += 1;
                                if dropped_frames % 120 == 1 {
                                    tracing::debug!(
                                        "Dropping screencast frames due to backpressure (dropped={})",
                                        dropped_frames
                                    );
                                }
                            }
                            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        // Timed out or failed; skip this frame and keep the loop alive.
                        tracing::trace!("Screencast frame skipped: {}", e);
                    }
                }
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

        let result = with_timeout(CDP_TIMEOUT, "evaluate", page.evaluate(script)).await?;
        let value: serde_json::Value = result.into_value()?;
        Ok(value.to_string())
    }

    async fn install_page_bridge(
        &self,
        binding_name: &str,
        init_script: &str,
    ) -> Result<mpsc::Receiver<String>> {
        let page = self.page().await?;

        // Runtime.addBinding (without a context id) registers the name on all
        // current and future execution contexts of this target, so the page can
        // call it from any later document.
        with_timeout(
            CDP_TIMEOUT,
            "addBinding",
            page.execute(AddBindingParams::new(binding_name)),
        )
        .await?;

        // The init script runs before page scripts on every subsequent document,
        // letting the bridge hook the game before it opens its WebSocket.
        with_timeout(
            CDP_TIMEOUT,
            "evaluateOnNewDocument",
            page.evaluate_on_new_document(init_script.to_string()),
        )
        .await?;

        let mut events = page
            .event_listener::<EventBindingCalled>()
            .await
            .map_err(|e| anyhow!("Failed to subscribe binding events: {}", e))?;

        let name = binding_name.to_string();
        let session_id = self.session_id.clone();
        let (tx, rx) = mpsc::channel(512);
        // Fire-and-forget forwarder: ends when the events stream closes (browser
        // teardown) or when the receiver side is dropped. Dropping the JoinHandle
        // intentionally detaches the task.
        drop(tokio::spawn(async move {
            while let Some(ev) = events.next().await {
                if ev.name != name {
                    continue;
                }
                match tx.try_send(ev.payload.clone()) {
                    Ok(()) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        tracing::warn!(
                            session_id = %session_id,
                            "Protocol bridge channel full, dropping message"
                        );
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                }
            }
        }));

        Ok(rx)
    }

    async fn capture_screen(&self) -> Result<DynamicImage> {
        let page = self.page().await?;

        let params = CaptureScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .build();
        let data = with_timeout(CDP_TIMEOUT, "capture_screen", page.screenshot(params)).await?;

        let img = image::load_from_memory(&data)?;
        Ok(img)
    }

    async fn input_text(&self, selector: &str, text: &str) -> Result<()> {
        let page = self.page().await?;
        let _input = self.input_lock.lock().await;

        // Find element and type text
        let element =
            with_timeout(CDP_TIMEOUT, "find_element", page.find_element(selector)).await?;
        with_timeout(CDP_TIMEOUT, "element click", async {
            element.click().await.map(|_| ())
        })
        .await?;
        with_timeout(CDP_TIMEOUT, "element type", async {
            element.type_str(text).await.map(|_| ())
        })
        .await?;

        tracing::debug!("Input text into {}", selector);
        Ok(())
    }

    async fn click_element(&self, selector: &str) -> Result<()> {
        let page = self.page().await?;
        let _input = self.input_lock.lock().await;

        let element =
            with_timeout(CDP_TIMEOUT, "find_element", page.find_element(selector)).await?;
        with_timeout(CDP_TIMEOUT, "element click", async {
            element.click().await.map(|_| ())
        })
        .await?;

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

            match with_timeout(CDP_TIMEOUT, "find_element", page.find_element(selector)).await {
                Ok(_) => {
                    tracing::debug!("Element {} is visible", selector);
                    return Ok(());
                }
                Err(_) => {
                    sleep(Duration::from_millis(200)).await;
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
        self.click_element("#form1 > div.r06 > div.login_box3 > p > input")
            .await?;

        tracing::info!("Password login form submitted");
        Ok(())
    }

    async fn refresh(&self) -> Result<()> {
        let page = self.page().await?;

        with_timeout(NAV_TIMEOUT, "refresh", async {
            page.reload().await.map(|_| ())
        })
        .await?;

        tracing::info!("Page refreshed");
        Ok(())
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        let page = self.page().await?;
        let _input = self.input_lock.lock().await;

        with_timeout(
            CDP_TIMEOUT,
            "insert_text",
            page.execute(InsertTextParams::new(text)),
        )
        .await?;

        tracing::debug!("Inserted text ({} chars)", text.len());
        Ok(())
    }
}
