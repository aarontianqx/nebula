use crate::application::command::SessionCommand;
use crate::application::eventbus::SharedEventBus;
use crate::domain::event::DomainEvent;
use crate::domain::model::{Account, SessionInfo, SessionState};
use crate::infrastructure::browser::{BrowserDriver, ChromiumDriver};
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
    browser: Box<dyn BrowserDriver + Send>,
    frame_rx: mpsc::UnboundedReceiver<String>,
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
        let browser = Box::new(ChromiumDriver::new(frame_tx));

        Self {
            id,
            account,
            state: SessionState::Idle,
            cmd_rx,
            event_bus,
            browser,
            frame_rx,
        }
    }

    /// Create a new session and return a handle
    pub fn spawn(
        account: Account,
        event_bus: SharedEventBus,
    ) -> SessionHandle {
        let id = uuid::Uuid::new_v4().to_string();
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
                            self.start_session().await;
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
            }
        }

        // Main command loop
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
            }
        }

        self.cleanup().await;
    }

    async fn start_session(&mut self) {
        self.transition_to(SessionState::Starting).await;

        // Start browser
        if let Err(e) = self.browser.start().await {
            tracing::error!("Failed to start browser: {}", e);
            self.transition_to(SessionState::Stopped).await;
            return;
        }

        // Navigate to game URL
        let game_url = "https://www.example.com"; // Placeholder URL
        if let Err(e) = self.browser.navigate(game_url).await {
            tracing::error!("Failed to navigate: {}", e);
            self.transition_to(SessionState::Stopped).await;
            return;
        }

        self.transition_to(SessionState::LoggingIn).await;

        // Start screencast
        if let Err(e) = self.browser.start_screencast().await {
            tracing::warn!("Failed to start screencast: {}", e);
        }

        // Perform login
        match self.perform_login().await {
            Ok(()) => {
                self.transition_to(SessionState::Ready).await;
                self.event_bus.publish(DomainEvent::LoginSucceeded {
                    session_id: self.id.clone(),
                });
            }
            Err(e) => {
                tracing::error!("Login failed: {}", e);
                self.event_bus.publish(DomainEvent::LoginFailed {
                    session_id: self.id.clone(),
                    reason: e.to_string(),
                });
                // Stay in LoggingIn state for manual intervention
            }
        }
    }

    async fn handle_command(&mut self, cmd: SessionCommand) -> bool {
        match cmd {
            SessionCommand::Stop => {
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

    async fn perform_login(&mut self) -> anyhow::Result<()> {
        // Check for existing cookies
        if let Some(cookies) = &self.account.cookies {
            if !cookies.is_empty() {
                self.browser.set_cookies(cookies).await?;
                // Refresh page to use cookies
                self.browser.navigate("https://www.example.com").await?;
                // TODO: Verify login succeeded
                return Ok(());
            }
        }

        // Manual login required - stay in LoggingIn state
        // User will interact with the canvas to complete login
        tracing::info!("Manual login required for account {}", self.account.id);
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

    async fn cleanup(&mut self) {
        tracing::info!("Session {} cleaning up", self.id);

        if let Err(e) = self.browser.stop().await {
            tracing::warn!("Failed to stop browser: {}", e);
        }

        self.event_bus.publish(DomainEvent::SessionStopped {
            session_id: self.id.clone(),
        });
    }
}

