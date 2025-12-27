use crate::application::command::SessionCommand;
use crate::application::eventbus::SharedEventBus;
use crate::application::service::{SessionActor, SessionHandle};
use crate::domain::repository::AccountRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Coordinator manages multiple SessionActors
pub struct Coordinator<R: AccountRepository> {
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    event_bus: SharedEventBus,
    account_repo: Arc<R>,
}

impl<R: AccountRepository + 'static> Coordinator<R> {
    pub fn new(event_bus: SharedEventBus, account_repo: Arc<R>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            account_repo,
        }
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

    /// Drag on all active sessions
    pub async fn drag_all(&self, from: (f64, f64), to: (f64, f64)) {
        let sessions = self.sessions.read().await;
        for handle in sessions.values() {
            let _ = handle.cmd_tx.send(SessionCommand::Drag { from, to }).await;
        }
    }

    /// Get all session infos
    pub async fn get_sessions(&self) -> Vec<crate::domain::model::SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|h| h.info.clone()).collect()
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

