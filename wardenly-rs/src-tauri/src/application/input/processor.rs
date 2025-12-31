use tokio::sync::RwLock;

/// Click event to be sent to coordinator (kept for compatibility)
#[derive(Debug, Clone)]
pub struct ClickEvent {
    pub session_id: String,
    pub x: f64,
    pub y: f64,
}

/// Input event processor that manages keyboard passthrough state
/// Note: Keyboard listening is now handled in the frontend (React)
pub struct InputEventProcessor {
    enabled: std::sync::Arc<RwLock<bool>>,
}

impl InputEventProcessor {
    pub fn new() -> Self {
        Self {
            enabled: std::sync::Arc::new(RwLock::new(false)),
        }
    }

    /// Enable or disable keyboard passthrough
    pub async fn set_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        *self.enabled.write().await = enabled;
        if enabled {
            tracing::info!("Keyboard passthrough enabled (frontend mode)");
        } else {
            tracing::info!("Keyboard passthrough disabled");
        }
        Ok(())
    }

    /// Check if keyboard passthrough is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }
}

impl Default for InputEventProcessor {
    fn default() -> Self {
        Self::new()
    }
}
