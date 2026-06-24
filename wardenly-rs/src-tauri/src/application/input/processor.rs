use tokio::sync::RwLock;

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
}

impl Default for InputEventProcessor {
    fn default() -> Self {
        Self::new()
    }
}
