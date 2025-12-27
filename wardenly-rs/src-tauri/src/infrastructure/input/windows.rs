use tokio::sync::mpsc;

use super::keyboard::{KeyboardListener, RawKeyEvent};

/// Windows keyboard listener (placeholder)
pub struct WindowsKeyboardListener {
    rx: Option<mpsc::UnboundedReceiver<RawKeyEvent>>,
}

impl WindowsKeyboardListener {
    pub fn new() -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self { rx: Some(rx) }
    }
}

impl Default for WindowsKeyboardListener {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardListener for WindowsKeyboardListener {
    fn start(&mut self) -> anyhow::Result<()> {
        tracing::warn!("Windows keyboard listener not fully implemented yet");
        // TODO: Implement Windows keyboard listener using rdev
        Ok(())
    }

    fn stop(&mut self) {
        // No-op for now
    }

    fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<RawKeyEvent>> {
        self.rx.take()
    }

    fn is_listening(&self) -> bool {
        false
    }
}

