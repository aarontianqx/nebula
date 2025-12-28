use crate::application::command::SessionCommand;
use crate::application::eventbus::SharedEventBus;
use crate::application::service::{SessionActor, SessionHandle};
use crate::domain::event::DomainEvent;
use crate::domain::repository::AccountRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Coordinator manages multiple SessionActors
pub struct Coordinator {
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    event_bus: SharedEventBus,
    account_repo: Arc<dyn AccountRepository>,
}

impl Coordinator {
    pub fn new(event_bus: SharedEventBus, account_repo: Arc<dyn AccountRepository>) -> Self {
        let coordinator = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            account_repo,
        };
        
        coordinator
    }
    
    /// Start the event listener for state sync and auto-cleanup of stopped sessions
    /// This should be called after the Coordinator is created and wrapped in Arc
    pub fn start_event_listener(self: &Arc<Self>) {
        let sessions = self.sessions.clone();
        let mut receiver = self.event_bus.subscribe();
        
        tauri::async_runtime::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        match event {
                            DomainEvent::SessionStateChanged { session_id, new_state, .. } => {
                                // Sync session state to SessionHandle
                                let mut sessions_guard = sessions.write().await;
                                if let Some(handle) = sessions_guard.get_mut(&session_id) {
                                    handle.info.state = new_state;
                                    tracing::trace!(
                                        "Synced session {} state to {:?}",
                                        session_id, new_state
                                    );
                                }
                            }
                            DomainEvent::SessionStopped { session_id } => {
                                // Auto-remove stopped session from coordinator
                                let mut sessions_guard = sessions.write().await;
                                if sessions_guard.remove(&session_id).is_some() {
                                    tracing::info!(
                                        "Auto-removed stopped session {} from coordinator",
                                        session_id
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Coordinator event listener lagged by {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::info!("Event bus closed, stopping coordinator event listener");
                        break;
                    }
                }
            }
        });
        
        tracing::info!("Coordinator event listener started");
    }

    /// Create a session for an account
    pub async fn create_session(&self, account_id: &str) -> anyhow::Result<String> {
        // Check if session already exists for this account
        {
            let sessions = self.sessions.read().await;
            for handle in sessions.values() {
                if handle.info.account_id == account_id {
                    anyhow::bail!("Session already exists for account {}", account_id);
                }
            }
        }

        // Get account from repository
        let account = self
            .account_repo
            .find_by_id(account_id)?
            .ok_or_else(|| anyhow::anyhow!("Account not found: {}", account_id))?;

        // Spawn session actor
        let handle = SessionActor::spawn(account, self.event_bus.clone());
        let session_id = handle.id.clone();

        // Store handle
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), handle);
        }

        tracing::info!("Created session {} for account {}", session_id, account_id);
        Ok(session_id)
    }

    /// Start a session
    pub async fn start_session(&self, session_id: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::Start)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send start command"))?;

        Ok(())
    }

    /// Stop a session
    pub async fn stop_session(&self, session_id: &str) -> anyhow::Result<()> {
        let handle = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id)
        };

        if let Some(handle) = handle {
            let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
            tracing::info!("Stopped session {}", session_id);
        }

        Ok(())
    }

    /// Stop all sessions
    pub async fn stop_all(&self) {
        let handles: Vec<SessionHandle> = {
            let mut sessions = self.sessions.write().await;
            sessions.drain().map(|(_, h)| h).collect()
        };

        for handle in handles {
            let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
        }

        tracing::info!("Stopped all sessions");
    }

    /// Send click to a specific session
    pub async fn click_session(&self, session_id: &str, x: f64, y: f64) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::Click { x, y })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send click command"))?;

        Ok(())
    }

    /// Send drag to a specific session
    pub async fn drag_session(
        &self,
        session_id: &str,
        from: (f64, f64),
        to: (f64, f64),
    ) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::Drag { from, to })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send drag command"))?;

        Ok(())
    }

    /// Click on all active sessions
    pub async fn click_all(&self, x: f64, y: f64) {
        let sessions = self.sessions.read().await;
        for handle in sessions.values() {
            let _ = handle.cmd_tx.send(SessionCommand::Click { x, y }).await;
        }
    }

    /// Get all session infos (states are kept in sync via event listener)
    pub async fn get_sessions(&self) -> Vec<crate::domain::model::SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|h| h.info.clone()).collect()
    }

    /// Start script on a specific session
    pub async fn start_script(&self, session_id: &str, script_name: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::StartScript {
                script_name: script_name.to_string(),
            })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send start script command"))?;

        tracing::info!("Started script {} on session {}", script_name, session_id);
        Ok(())
    }

    /// Stop script on a specific session
    pub async fn stop_script(&self, session_id: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::StopScript)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send stop script command"))?;

        tracing::info!("Stopped script on session {}", session_id);
        Ok(())
    }

    /// Start script on all sessions
    pub async fn start_all_scripts(&self, script_name: &str) {
        let sessions = self.sessions.read().await;
        for (session_id, handle) in sessions.iter() {
            if handle
                .cmd_tx
                .send(SessionCommand::StartScript {
                    script_name: script_name.to_string(),
                })
                .await
                .is_ok()
            {
                tracing::info!("Started script {} on session {}", script_name, session_id);
            }
        }
    }

    /// Stop scripts on all sessions
    pub async fn stop_all_scripts(&self) {
        let sessions = self.sessions.read().await;
        for (session_id, handle) in sessions.iter() {
            if handle.cmd_tx.send(SessionCommand::StopScript).await.is_ok() {
                tracing::info!("Stopped script on session {}", session_id);
            }
        }
    }
    
    /// Refresh/reload page on a specific session
    pub async fn refresh_session(&self, session_id: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::Refresh)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send refresh command"))?;

        tracing::info!("Refreshed session {}", session_id);
        Ok(())
    }
    
    /// Start screencast streaming on a specific session
    pub async fn start_screencast(&self, session_id: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::StartScreencast)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send start screencast command"))?;

        tracing::info!("Started screencast for session {}", session_id);
        Ok(())
    }
    
    /// Stop screencast streaming on a specific session
    pub async fn stop_screencast(&self, session_id: &str) -> anyhow::Result<()> {
        let sessions = self.sessions.read().await;
        let handle = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        handle
            .cmd_tx
            .send(SessionCommand::StopScreencast)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send stop screencast command"))?;

        tracing::info!("Stopped screencast for session {}", session_id);
        Ok(())
    }
}
