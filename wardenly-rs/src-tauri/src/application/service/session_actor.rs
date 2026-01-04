use crate::application::command::SessionCommand;
use crate::application::eventbus::SharedEventBus;
use crate::application::service::script_runner::{ScriptHandle, ScriptRunner};
use crate::domain::event::DomainEvent;
use crate::domain::model::{Account, SessionInfo, SessionState};
use crate::infrastructure::browser::{BrowserDriver, ChromiumDriver};
use crate::infrastructure::config::resources;
use crate::infrastructure::ocr::global_ocr_client;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
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

        // Perform login
        match self.perform_login().await {
            Ok(()) => {
                // Wait for game to load
                match self.wait_for_game_load().await {
                    Ok(()) => {
                        self.transition_to(SessionState::Ready).await;
                        self.event_bus.publish(DomainEvent::LoginSucceeded {
                            session_id: self.id.clone(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Wait for game load failed: {}", e);
                        // Still transition to Ready for manual intervention
                        self.transition_to(SessionState::Ready).await;
                    }
                }
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
            SessionCommand::StopScript => {
                self.stop_script().await;
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

    async fn perform_login(&mut self) -> anyhow::Result<()> {
        let game_url = self.game_url();
        let login_timeout = std::time::Duration::from_secs(20);

        // Check for existing cookies
        let has_cookies = self.account.cookies.as_ref().map(|c| !c.is_empty()).unwrap_or(false);
        
        if has_cookies {
            let cookies = self.account.cookies.clone().unwrap();
            tracing::info!("Attempting login with cookies for {}", self.account.identity());
            
            // Use atomic login method from BrowserDriver
            match self.browser.login_with_cookies(&game_url, &cookies, login_timeout).await {
                Ok(()) => {
                    tracing::info!("Cookie login succeeded for {}", self.account.identity());
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Cookie login failed for {}: {}, falling back to password", self.account.identity(), e);
                    // Fall through to password login
                }
            }
        }

        // Login with username/password using atomic method
        tracing::info!("Attempting login with password for {}", self.account.identity());
        self.browser.login_with_password(
            &game_url,
            &self.account.user_name,
            &self.account.password,
            login_timeout,
        ).await?;
        
        // Save cookies after successful password login
        self.save_cookies_after_login().await;
        
        Ok(())
    }
    
    async fn save_cookies_after_login(&mut self) {
        match self.browser.get_cookies().await {
            Ok(cookies) => {
                tracing::info!("Captured {} cookies after login", cookies.len());
                self.account.cookies = Some(cookies);
                // Note: cookies should be persisted to storage here
                // For now, we just keep them in memory for the session
            }
            Err(e) => {
                tracing::warn!("Failed to save cookies after login: {}", e);
            }
        }
    }

    async fn wait_for_game_load(&mut self) -> anyhow::Result<()> {
        const MAX_ATTEMPTS: u32 = 10;
        const WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

        let scenes = resources::load_scenes().unwrap_or_default();

        for i in 0..MAX_ATTEMPTS {
            tracing::info!(
                "Waiting for game to load (attempt {}/{})",
                i + 1,
                MAX_ATTEMPTS
            );

            tokio::time::sleep(WAIT_INTERVAL).await;

            // Capture screen
            let screen = match self.browser.capture_screen().await {
                Ok(img) => img,
                Err(e) => {
                    tracing::warn!("Failed to capture screen: {}", e);
                    continue;
                }
            };

            // Check for known scenes: user_agreement or main_city
            if let Some(scene) = resources::find_scene(&scenes, "user_agreement") {
                if scene.matches(&screen) {
                    tracing::info!("Detected user_agreement scene, clicking agree");
                    if let Some(action) = scene.actions.get("Agree") {
                        if let crate::domain::model::SceneAction::Click { point } = action {
                            self.browser.click(point.x as f64, point.y as f64).await?;
                        }
                    }
                    return Ok(());
                }
            }

            if let Some(scene) = resources::find_scene(&scenes, "main_city") {
                if scene.matches(&screen) {
                    tracing::info!("Detected main_city scene, game loaded successfully");
                    return Ok(());
                }
            }
        }

        anyhow::bail!("Timeout waiting for game to load after {} attempts", MAX_ATTEMPTS)
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

        // Create script runner (uses global OCR client singleton)
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

        self.script_handle = Some(ScriptHandle { cmd_tx });
        self.transition_to(SessionState::ScriptRunning).await;

        let session_id = self.id.clone();
        let script_name_for_spawn = script.name.clone();
        let script_name_for_log = script.name.clone();
        let event_bus = self.event_bus.clone();

        // Spawn script runner
        tokio::spawn(async move {
            let reason = runner.run().await;
            tracing::info!(
                session_id = %session_id,
                script = %script_name_for_spawn,
                reason = ?reason,
                "Script finished"
            );

            // Publish script stopped event
            event_bus.publish(DomainEvent::ScriptStopped {
                session_id,
                script_name: script_name_for_spawn,
            });
        });

        tracing::info!("Started script: {}", script_name_for_log);
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

