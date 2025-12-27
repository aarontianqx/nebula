use tokio::sync::mpsc;

use super::keyboard::{KeyboardListener, RawKeyEvent};

/// Linux keyboard listener (placeholder)
pub struct LinuxKeyboardListener {
    rx: Option<mpsc::UnboundedReceiver<RawKeyEvent>>,
}

impl LinuxKeyboardListener {
    pub fn new() -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self { rx: Some(rx) }
    }
}

impl Default for LinuxKeyboardListener {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardListener for LinuxKeyboardListener {
    fn start(&mut self) -> anyhow::Result<()> {
        tracing::warn!("Linux keyboard listener not fully implemented yet");
        // TODO: Implement Linux keyboard listener using rdev
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

