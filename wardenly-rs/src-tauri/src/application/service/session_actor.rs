use crate::application::command::SessionCommand;
use crate::application::eventbus::SharedEventBus;
use crate::application::service::script_runner::{ScriptHandle, ScriptRunner};
use crate::domain::event::DomainEvent;
use crate::domain::model::{Account, Scene, SceneAction, SessionInfo, SessionState};
use crate::infrastructure::browser::{BrowserDriver, ChromiumDriver};
use crate::infrastructure::config::resources;
use crate::infrastructure::ocr::global_ocr_client;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Layer 1: find the iframe pointing at the layer-2 server entry page.
const JS_FIND_SERVER_ENTRY_IFRAME: &str = r"(() => {
    const src = Array.from(document.querySelectorAll('iframe'))
        .map(f => f.src)
        .find(s => s.includes('.wly.h5.lequ.com/index.php'));
    return src || null;
})()";

/// Layer 2: read the game page URL (layer 3, short-lived content ticket).
const JS_READ_GAME_IFRAME_SRC: &str = r"(() => {
    const f = document.getElementById('gameIframe');
    return f && f.src ? f.src : null;
})()";

/// Layer 3: the game's own Connection singleton reports connected.
const JS_GAME_CONNECTED: &str = r"(() => {
    try {
        if (typeof window.__require !== 'function') return false;
        return window.__require('Connection').default.get()._connected === true;
    } catch (e) { return false; }
})()";

/// Layer 3: the injected page bridge finished hooking the game's Connection.
const JS_BRIDGE_READY: &str = r"!!(window.__wardenly && window.__wardenly.ready === true)";

/// CDP Runtime binding name the page bridge uses to push protocol messages.
/// Must match the call site in resources/page_bridge.js.
const BRIDGE_BINDING_NAME: &str = "__wardenlyReport";

/// In-page bridge, injected via Page.addScriptToEvaluateOnNewDocument.
const PAGE_BRIDGE_JS: &str = include_str!("../../../resources/page_bridge.js");

/// Wire format of a message pushed by the page bridge.
#[derive(Debug, serde::Deserialize)]
struct BridgeMessage {
    id: u32,
    name: Option<String>,
    data: serde_json::Value,
}

/// Handle to communicate with a SessionActor
pub struct SessionHandle {
    pub id: String,
    pub info: SessionInfo,
    pub cmd_tx: mpsc::Sender<SessionCommand>,
}

/// SessionActor manages a single browser session
pub struct SessionActor {
    id: String,
    account: Account,
    state: SessionState,
    cmd_rx: mpsc::Receiver<SessionCommand>,
    event_bus: SharedEventBus,
    browser: Arc<dyn BrowserDriver + Send + Sync>,
    frame_rx: mpsc::Receiver<String>,
    script_handle: Option<ScriptHandle>,
    /// Forwarder turning page-bridge pushes into ProtocolMessage events.
    protocol_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SessionActor {
    pub fn new(
        id: String,
        account: Account,
        cmd_rx: mpsc::Receiver<SessionCommand>,
        event_bus: SharedEventBus,
        frame_tx: mpsc::Sender<String>,
        frame_rx: mpsc::Receiver<String>,
    ) -> Self {
        // Pass session ID and account ID to browser driver for persistent profile directory
        let browser = Arc::new(ChromiumDriver::new(&id, &account.id, frame_tx));

        Self {
            id,
            account,
            state: SessionState::Idle,
            cmd_rx,
            event_bus,
            browser,
            frame_rx,
            script_handle: None,
            protocol_handle: None,
        }
    }

    /// Create a new session and return a handle
    pub fn spawn(account: Account, event_bus: SharedEventBus) -> SessionHandle {
        let id = ulid::Ulid::new().to_string();
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        // Keep only a tiny buffer for screencast frames to avoid unbounded memory growth.
        // New frames can be dropped by producer when consumer is busy.
        let (frame_tx, frame_rx) = mpsc::channel(2);

        let info = SessionInfo {
            id: id.clone(),
            account_id: account.id.clone(),
            display_name: format!("{} - {}", account.server_id, account.role_name),
            state: SessionState::Idle,
        };

        let actor = Self::new(
            id.clone(),
            account,
            cmd_rx,
            event_bus.clone(),
            frame_tx,
            frame_rx,
        );

        // Publish session created event
        event_bus.publish(DomainEvent::SessionCreated {
            session_id: info.id.clone(),
            account_id: info.account_id.clone(),
            display_name: info.display_name.clone(),
        });

        // Spawn the actor
        tokio::spawn(actor.run());

        SessionHandle { id, info, cmd_tx }
    }

    /// Main run loop
    pub async fn run(mut self) {
        tracing::info!(
            "Session {} started for account {}",
            self.id,
            self.account.id
        );

        // Wait for Start command
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    match cmd {
                        SessionCommand::Start => {
                            // start_session returns false if it failed and session should stop
                            if !self.start_session().await {
                                self.cleanup().await;
                                return;
                            }
                            break;
                        }
                        SessionCommand::Stop => {
                            self.cleanup().await;
                            return;
                        }
                        _ => {
                            tracing::warn!("Received command {:?} before Start", cmd);
                        }
                    }
                }
                // If command channel closes, cleanup and exit
                else => {
                    tracing::warn!("Session {} command channel closed before start", self.id);
                    self.cleanup().await;
                    return;
                }
            }
        }

        // Main command loop - only reached if start_session succeeded
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    if !self.handle_command(cmd).await {
                        break;
                    }
                }
                Some(frame) = self.frame_rx.recv() => {
                    self.handle_frame(frame).await;
                }
                // If channels close, exit loop
                else => {
                    tracing::warn!("Session {} channels closed", self.id);
                    break;
                }
            }
        }

        self.cleanup().await;
    }

    /// Start the session. Returns true if successful, false if failed.
    async fn start_session(&mut self) -> bool {
        self.transition_to(SessionState::Starting).await;

        // Start browser
        if let Err(e) = self.browser.start().await {
            tracing::error!("Failed to start browser for session {}: {}", self.id, e);
            self.transition_to(SessionState::Stopped).await;
            self.event_bus.publish(DomainEvent::LoginFailed {
                session_id: self.id.clone(),
                reason: format!("Browser failed to start: {}", e),
            });
            return false;
        }

        self.transition_to(SessionState::LoggingIn).await;

        // NOTE: Screencast is NOT started automatically here.
        // It is controlled by the frontend via StartScreencast/StopScreencast commands.
        // This ensures the UI's screencast checkbox state is respected.

        // Perform login using race-based detection
        match self.perform_login().await {
            Ok(()) => {
                self.transition_to(SessionState::Ready).await;
                self.event_bus.publish(DomainEvent::LoginSucceeded {
                    session_id: self.id.clone(),
                });
            }
            Err(e) => {
                tracing::error!("Login failed for session {}: {}", self.id, e);
                self.event_bus.publish(DomainEvent::LoginFailed {
                    session_id: self.id.clone(),
                    reason: e.to_string(),
                });
                // Transition to Ready for manual intervention
                self.transition_to(SessionState::Ready).await;
            }
        }

        true
    }

    async fn handle_command(&mut self, cmd: SessionCommand) -> bool {
        match cmd {
            SessionCommand::Stop => {
                // Stop script if running
                self.stop_script().await;
                self.transition_to(SessionState::Stopped).await;
                return false;
            }
            SessionCommand::Click { x, y } => {
                if self.state.can_accept_interaction() {
                    if let Err(e) = self.browser.click(x, y).await {
                        tracing::warn!("Click failed: {}", e);
                    }
                }
            }
            SessionCommand::Drag { from, to } => {
                if self.state.can_accept_interaction() {
                    if let Err(e) = self.browser.drag(from, to).await {
                        tracing::warn!("Drag failed: {}", e);
                    }
                }
            }
            SessionCommand::StartScreencast => {
                if let Err(e) = self.browser.start_screencast().await {
                    tracing::warn!("Start screencast failed: {}", e);
                }
            }
            SessionCommand::StopScreencast => {
                if let Err(e) = self.browser.stop_screencast().await {
                    tracing::warn!("Stop screencast failed: {}", e);
                }
            }
            SessionCommand::Start => {
                tracing::warn!("Session already started");
            }
            SessionCommand::StartScript { script_name } => {
                self.start_script(&script_name).await;
            }
            SessionCommand::StopScript { run_id } => {
                // If run_id is provided, only stop if it matches the current script
                // This prevents stale events from stopping newly started scripts
                if let Some(expected_run_id) = run_id {
                    if let Some(handle) = &self.script_handle {
                        if handle.run_id == expected_run_id {
                            self.stop_script().await;
                        } else {
                            tracing::debug!(
                                "Ignoring StopScript: run_id mismatch (expected={}, current={})",
                                expected_run_id,
                                handle.run_id
                            );
                        }
                    }
                    // If no script is running, ignore silently
                } else {
                    // No run_id provided = unconditional stop (user action)
                    self.stop_script().await;
                }
            }
            SessionCommand::Refresh => {
                if self.state.can_accept_interaction() {
                    if let Err(e) = self.browser.refresh().await {
                        tracing::warn!("Refresh failed: {}", e);
                    }
                }
            }
            SessionCommand::CaptureScreenshot => {
                // Capture a single screenshot and send it as a frame
                // Used when screencast is off but user wants to see current state
                if self.state.can_accept_interaction() {
                    match self.browser.capture_screen().await {
                        Ok(img) => {
                            // Encode as JPEG base64
                            let mut buffer = std::io::Cursor::new(Vec::new());
                            if let Err(e) = img.write_to(&mut buffer, image::ImageFormat::Jpeg) {
                                tracing::warn!("Failed to encode screenshot: {}", e);
                            } else {
                                use base64::Engine;
                                let base64_data = base64::engine::general_purpose::STANDARD
                                    .encode(buffer.into_inner());
                                self.handle_frame(base64_data).await;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Screenshot capture failed: {}", e);
                        }
                    }
                }
            }
            SessionCommand::InsertText { text } => {
                if self.state.can_accept_interaction() {
                    if let Err(e) = self.browser.insert_text(&text).await {
                        tracing::warn!("Insert text failed: {}", e);
                    }
                }
            }
            SessionCommand::SendProtocol { name, payload } => {
                if self.state.can_accept_interaction() {
                    if let Err(e) = self.send_protocol(&name, &payload).await {
                        tracing::warn!("SendProtocol {} failed: {}", name, e);
                    }
                }
            }
        }
        true
    }

    async fn handle_frame(&self, frame: String) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        self.event_bus.publish(DomainEvent::ScreencastFrame {
            session_id: self.id.clone(),
            image_base64: frame,
            timestamp,
        });
    }

    /// Build the layer-1 entry URL (account login page) for this account's server.
    fn entry_url(&self) -> String {
        let s = &self.account.server_id;
        format!("http://www.lequ.com/server/wly/s/{}/ish5/{}", s, s)
    }

    /// Evaluate JS that returns a string or null, and extract the string.
    async fn eval_string(&self, script: &str) -> Option<String> {
        let raw = self.browser.evaluate(script).await.ok()?;
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()?
            .as_str()
            .map(|s| s.to_string())
    }

    /// Evaluate JS that returns a boolean.
    async fn eval_bool(&self, script: &str) -> bool {
        self.browser
            .evaluate(script)
            .await
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Send a protocol message through the page bridge.
    async fn send_protocol(&self, name: &str, payload: &serde_json::Value) -> anyhow::Result<()> {
        // serde_json string escaping produces a valid JS string literal;
        // a JSON value is directly a valid JS expression.
        let name_literal = serde_json::to_string(name)?;
        let script = format!(
            "window.__wardenly ? window.__wardenly.send({}, {}) : 'ERR bridge not installed'",
            name_literal, payload
        );
        let result = self.browser.evaluate(&script).await?;
        if result.contains("ERR") {
            anyhow::bail!("bridge rejected send: {}", result);
        }
        tracing::debug!("send_protocol {} -> {}", name, result);
        Ok(())
    }

    /// Forward page-bridge pushes to the event bus as ProtocolMessage events.
    fn spawn_protocol_forwarder(&mut self, mut rx: mpsc::Receiver<String>) {
        let event_bus = self.event_bus.clone();
        let session_id = self.id.clone();
        let handle = tokio::spawn(async move {
            while let Some(raw) = rx.recv().await {
                match serde_json::from_str::<BridgeMessage>(&raw) {
                    Ok(msg) => {
                        event_bus.publish(DomainEvent::ProtocolMessage {
                            session_id: session_id.clone(),
                            protocol_id: msg.id,
                            name: msg.name,
                            data: msg.data,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            session_id = %session_id,
                            "Failed to parse bridge message: {} ({} bytes)",
                            e,
                            raw.len()
                        );
                    }
                }
            }
        });
        self.protocol_handle = Some(handle);
    }

    /// Perform login by walking the three-layer entry chain with pure DOM operations:
    ///
    ///   [1] www.lequ.com login page (fill form, or reuse cached profile)
    ///   [2] s{server}.wly.h5.lequ.com server entry page (ticket in URL)
    ///   [3] s1res.lequ.com game page (short-lived content ticket in iframe src)
    ///
    /// Layer 3 is opened directly as the top-level page: cross-origin iframes are
    /// inaccessible to `evaluate`, and the game JS context is required both for the
    /// ready criterion (`Connection._connected`) and for later protocol driving.
    async fn perform_login(&mut self) -> anyhow::Result<()> {
        let scenes = resources::load_scenes().unwrap_or_default();
        let entry_url = self.entry_url();

        tracing::info!(
            "Navigating to {} for {}",
            entry_url,
            self.account.identity()
        );
        self.browser.navigate(&entry_url).await?;

        // Layer 1 → 2: log in if the form shows up, then read the server-entry
        // iframe URL from the DOM.
        let server_entry_url = self
            .wait_for_server_entry_url(Duration::from_secs(30))
            .await?;

        // Layer 2 → 3: read the game iframe URL. The content ticket is short-lived
        // (~5 min), so it must be fetched fresh and used immediately.
        tracing::info!("Navigating to server entry page");
        self.browser.navigate(&server_entry_url).await?;
        let game_page_url = self.wait_for_game_page_url(Duration::from_secs(15)).await?;

        // Install the page bridge BEFORE the game page loads: the init script
        // must be registered before navigation, because the game opens its
        // WebSocket early in boot and the bridge hooks the Connection module.
        let protocol_rx = self
            .browser
            .install_page_bridge(BRIDGE_BINDING_NAME, PAGE_BRIDGE_JS)
            .await?;
        self.spawn_protocol_forwarder(protocol_rx);

        // HTTPS-First upgrades would force this page to https, where the game's
        // plaintext ws:// is blocked as mixed content. Fail loudly instead of
        // stalling on the loading screen with no traffic.
        tracing::info!("Navigating directly to game page");
        self.browser.navigate(&game_page_url).await?;
        let protocol = self.eval_string("location.protocol").await;
        if protocol.as_deref() != Some("http:") {
            anyhow::bail!(
                "Game page protocol is {:?}, expected \"http:\" (ws:// would be blocked as mixed content; check --disable-features=HttpsUpgrades)",
                protocol
            );
        }

        self.wait_for_game_connected(&scenes, Duration::from_secs(60))
            .await?;

        // The bridge hooks the same Connection the wait above observes, so it
        // should be ready almost immediately; a failure here means injection
        // broke, which must not pass silently (protocol driving would be dead).
        let bridge_wait = Instant::now();
        while !self.eval_bool(JS_BRIDGE_READY).await {
            if bridge_wait.elapsed() > Duration::from_secs(10) {
                anyhow::bail!("Page bridge failed to install on the game page");
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        tracing::info!("Page bridge installed");
        Ok(())
    }

    /// Wait for the layer-2 server entry URL to appear in a top-level iframe,
    /// submitting the login form first if it is present.
    async fn wait_for_server_entry_url(&mut self, timeout: Duration) -> anyhow::Result<String> {
        let start = Instant::now();
        let mut login_attempted = false;

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for server entry iframe");
            }

            if let Some(url) = self.eval_string(JS_FIND_SERVER_ENTRY_IFRAME).await {
                tracing::info!("Got server entry URL");
                return Ok(url);
            }

            if !login_attempted
                && self
                    .browser
                    .wait_visible("#username", Duration::from_millis(300))
                    .await
                    .is_ok()
            {
                tracing::info!(
                    "Detected login form, performing password login for {}",
                    self.account.identity()
                );
                self.browser
                    .login_with_password(
                        &self.account.user_name,
                        &self.account.password,
                        Duration::from_secs(10),
                    )
                    .await?;
                login_attempted = true;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Wait for the layer-2 page to write the game URL into `#gameIframe.src`.
    async fn wait_for_game_page_url(&self, timeout: Duration) -> anyhow::Result<String> {
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for game iframe URL");
            }

            if let Some(url) = self.eval_string(JS_READ_GAME_IFRAME_SRC).await {
                return Ok(url);
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    /// Wait until the game's own Connection singleton reports connected.
    /// The first-time user agreement is a canvas dialog with no DOM counterpart;
    /// it is the one place that still falls back to scene recognition + click.
    async fn wait_for_game_connected(
        &mut self,
        scenes: &[Scene],
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for game connection");
            }

            if self.eval_bool(JS_GAME_CONNECTED).await {
                tracing::info!("Game connection established, login complete");
                return Ok(());
            }

            if let Some(scene) = resources::find_scene(scenes, "user_agreement") {
                if let Ok(screen) = self.browser.capture_screen().await {
                    if scene.matches(&screen) {
                        tracing::info!("Detected user_agreement scene, clicking Agree");
                        self.click_scene_action(scene, "Agree").await?;
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Click a named action in a scene.
    async fn click_scene_action(&self, scene: &Scene, action_name: &str) -> anyhow::Result<()> {
        if let Some(SceneAction::Click { point }) = scene.actions.get(action_name) {
            self.browser.click(point.x as f64, point.y as f64).await?;
        }
        Ok(())
    }

    async fn transition_to(&mut self, new_state: SessionState) {
        let old_state = self.state;
        if old_state == new_state {
            return;
        }

        tracing::debug!(
            "Session {} state: {:?} -> {:?}",
            self.id,
            old_state,
            new_state
        );

        self.state = new_state;

        self.event_bus.publish(DomainEvent::SessionStateChanged {
            session_id: self.id.clone(),
            old_state,
            new_state,
        });
    }

    async fn start_script(&mut self, script_name: &str) {
        if self.state != SessionState::Ready {
            tracing::warn!("Cannot start script: session not ready");
            return;
        }

        // Stop existing script if any
        self.stop_script().await;

        // Load scripts
        let scripts = resources::load_scripts().unwrap_or_default();
        let script = match resources::find_script(&scripts, script_name) {
            Some(s) => s.clone(),
            None => {
                tracing::error!("Script not found: {}", script_name);
                return;
            }
        };

        // Load scenes
        let scenes = resources::load_scenes().unwrap_or_default();

        // Generate unique run_id for this script execution instance
        let run_id = ulid::Ulid::new().to_string();

        // Create shared running flag - this allows immediate stop signal propagation
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

        // Create script runner (uses global OCR client singleton)
        // Pass the shared running flag so runner checks it before operations
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let mut runner = ScriptRunner::new(
            self.id.clone(),
            script.clone(),
            scenes,
            self.browser.clone(),
            global_ocr_client(),
            self.event_bus.clone(),
            cmd_rx,
        );
        // Replace the internal running flag with our shared one
        runner.set_running_flag(running.clone());

        self.script_handle = Some(ScriptHandle {
            cmd_tx,
            running,
            run_id: run_id.clone(),
        });
        self.transition_to(SessionState::ScriptRunning).await;

        // Publish ScriptStarted event so Coordinator can track the current run_id
        self.event_bus.publish(DomainEvent::ScriptStarted {
            session_id: self.id.clone(),
            script_name: script.name.clone(),
            run_id: run_id.clone(),
        });

        let session_id = self.id.clone();
        let script_name_for_spawn = script.name.clone();
        let script_name_for_log = script.name.clone();
        let event_bus = self.event_bus.clone();
        let run_id_for_event = run_id.clone();

        // Spawn script runner
        tokio::spawn(async move {
            let reason = runner.run().await;
            tracing::info!(
                session_id = %session_id,
                script = %script_name_for_spawn,
                run_id = %run_id_for_event,
                reason = ?reason,
                "Script finished"
            );

            // Publish script stopped event with run_id for precise identification
            event_bus.publish(DomainEvent::ScriptStopped {
                session_id,
                script_name: script_name_for_spawn,
                run_id: run_id_for_event,
            });
        });

        tracing::info!(run_id = %run_id, "Started script: {}", script_name_for_log);
    }

    async fn stop_script(&mut self) {
        if let Some(handle) = self.script_handle.take() {
            handle.stop().await;
            tracing::info!("Stopped script");
        }

        if self.state == SessionState::ScriptRunning {
            self.transition_to(SessionState::Ready).await;
        }
    }

    async fn cleanup(&mut self) {
        tracing::info!("Session {} cleaning up", self.id);

        // Stop script if running
        self.stop_script().await;

        if let Some(handle) = self.protocol_handle.take() {
            handle.abort();
        }

        if let Err(e) = self.browser.stop().await {
            tracing::warn!("Failed to stop browser: {}", e);
        }

        self.event_bus.publish(DomainEvent::SessionStopped {
            session_id: self.id.clone(),
        });
    }
}
