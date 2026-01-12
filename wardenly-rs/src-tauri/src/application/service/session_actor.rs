use crate::application::command::SessionCommand;
use crate::application::eventbus::SharedEventBus;
use crate::application::service::script_runner::{ScriptHandle, ScriptRunner};
use crate::domain::model::{Account, Scene, SceneAction, SessionInfo, SessionState};
use crate::domain::event::DomainEvent;
use crate::infrastructure::browser::{BrowserDriver, ChromiumDriver};
use crate::infrastructure::config::resources;
use crate::infrastructure::ocr::global_ocr_client;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

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
    frame_rx: mpsc::UnboundedReceiver<String>,
    script_handle: Option<ScriptHandle>,
}

impl SessionActor {
    pub fn new(
        id: String,
        account: Account,
        cmd_rx: mpsc::Receiver<SessionCommand>,
        event_bus: SharedEventBus,
        frame_tx: mpsc::UnboundedSender<String>,
        frame_rx: mpsc::UnboundedReceiver<String>,
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
        }
    }

    /// Create a new session and return a handle
    pub fn spawn(
        account: Account,
        event_bus: SharedEventBus,
    ) -> SessionHandle {
        let id = ulid::Ulid::new().to_string();
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let (frame_tx, frame_rx) = mpsc::unbounded_channel();

        let info = SessionInfo {
            id: id.clone(),
            account_id: account.id.clone(),
            display_name: format!("{} - {}", account.server_id, account.role_name),
            state: SessionState::Idle,
        };

        let actor = Self::new(id.clone(), account, cmd_rx, event_bus.clone(), frame_tx, frame_rx);

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
        tracing::info!("Session {} started for account {}", self.id, self.account.id);

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
                                expected_run_id, handle.run_id
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

    /// Build the game URL for this account's server
    fn game_url(&self) -> String {
        format!(
            "http://www.lequ.com/server/wly/s/{}",
            self.account.server_id
        )
    }

    /// Perform login using race-based detection.
    /// Simultaneously waits for login form OR game scenes, handling whichever appears first.
    /// This gracefully handles cached browser profiles that skip login entirely.
    async fn perform_login(&mut self) -> anyhow::Result<()> {
        let game_url = self.game_url();
        let timeout = Duration::from_secs(30);
        let scene_check_interval = Duration::from_millis(500);
        let login_form_check_interval = Duration::from_millis(300);
        
        // Load scenes for detection
        let scenes = resources::load_scenes().unwrap_or_default();
        
        // Navigate to game URL first
        tracing::info!("Navigating to {} for {}", game_url, self.account.identity());
        self.browser.navigate(&game_url).await?;
        
        let start = Instant::now();
        let mut login_attempted = false;
        
        // Race loop: check for login form OR game scenes
        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Login timeout after {:?}", timeout);
            }
            
            // Check for game scenes first (higher priority - means we're already logged in)
            if let Some(matched_scene) = self.check_game_scenes(&scenes).await {
                match matched_scene.name.as_str() {
                    "user_agreement" => {
                        tracing::info!("Detected user_agreement scene, clicking Agree");
                        self.click_scene_action(&matched_scene, "Agree").await?;
                        // After clicking agree, wait for main_city scene
                        return self.wait_for_main_city(&scenes, timeout - start.elapsed()).await;
                    }
                    "main_city" | "main_city_shadow" => {
                        tracing::info!("Detected {} scene, already logged in", matched_scene.name);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            
            // Check for login form (only attempt login once)
            if !login_attempted {
                if self.browser.wait_visible("#username", login_form_check_interval).await.is_ok() {
                    tracing::info!("Detected login form, performing password login for {}", self.account.identity());
                    self.browser.login_with_password(
                        &self.account.user_name,
                        &self.account.password,
                        Duration::from_secs(10),
                    ).await?;
                    login_attempted = true;
                    // After login form submission, continue loop to wait for game scenes
                    continue;
                }
            }
            
            // Small delay before next check iteration
            tokio::time::sleep(scene_check_interval).await;
        }
    }
    
    /// Check for game scenes that indicate login success.
    /// Returns the matched scene if found.
    async fn check_game_scenes(&self, scenes: &[Scene]) -> Option<Scene> {
        let screen = match self.browser.capture_screen().await {
            Ok(img) => img,
            Err(_) => return None,
        };
        
        // Check scenes in priority order
        for scene_name in &["user_agreement", "main_city_shadow", "main_city"] {
            if let Some(scene) = resources::find_scene(scenes, scene_name) {
                if scene.matches(&screen) {
                    return Some(scene.clone());
                }
            }
        }
        
        None
    }
    
    /// Click a named action in a scene.
    async fn click_scene_action(&self, scene: &Scene, action_name: &str) -> anyhow::Result<()> {
        if let Some(action) = scene.actions.get(action_name) {
            if let SceneAction::Click { point } = action {
                self.browser.click(point.x as f64, point.y as f64).await?;
            }
        }
        Ok(())
    }
    
    /// Wait for main_city or main_city_shadow scene after user_agreement.
    async fn wait_for_main_city(&self, scenes: &[Scene], timeout: Duration) -> anyhow::Result<()> {
        let start = Instant::now();
        let check_interval = Duration::from_millis(500);
        
        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for main_city scene");
            }
            
            if let Some(matched) = self.check_game_scenes(scenes).await {
                match matched.name.as_str() {
                    "main_city" | "main_city_shadow" => {
                        tracing::info!("Detected {} scene, game loaded successfully", matched.name);
                        return Ok(());
                    }
                    "user_agreement" => {
                        // Still on agreement page, click again
                        tracing::debug!("Still on user_agreement, clicking Agree again");
                        self.click_scene_action(&matched, "Agree").await?;
                    }
                    _ => {}
                }
            }
            
            tokio::time::sleep(check_interval).await;
        }
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

        if let Err(e) = self.browser.stop().await {
            tracing::warn!("Failed to stop browser: {}", e);
        }

        self.event_bus.publish(DomainEvent::SessionStopped {
            session_id: self.id.clone(),
        });
    }
}

