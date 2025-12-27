use rdev::{listen, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

use super::keyboard::{KeyCode, KeyEventType, KeyboardListener, RawKeyEvent};

/// macOS keyboard listener using rdev
pub struct MacOSKeyboardListener {
    tx: mpsc::UnboundedSender<RawKeyEvent>,
    rx: Option<mpsc::UnboundedReceiver<RawKeyEvent>>,
    listening: Arc<AtomicBool>,
}

impl MacOSKeyboardListener {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx: Some(rx),
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for MacOSKeyboardListener {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardListener for MacOSKeyboardListener {
    fn start(&mut self) -> anyhow::Result<()> {
        if self.listening.load(Ordering::SeqCst) {
            return Ok(());
        }

        let tx = self.tx.clone();
        let listening = self.listening.clone();
        listening.store(true, Ordering::SeqCst);

        std::thread::spawn(move || {
            let callback = move |event: rdev::Event| {
                if !listening.load(Ordering::SeqCst) {
                    return;
                }

                let raw_event = match event.event_type {
                    EventType::KeyPress(key) => Some(RawKeyEvent {
                        key: convert_key(key),
                        event_type: KeyEventType::Press,
                        timestamp: Instant::now(),
                    }),
                    EventType::KeyRelease(key) => Some(RawKeyEvent {
                        key: convert_key(key),
                        event_type: KeyEventType::Release,
                        timestamp: Instant::now(),
                    }),
                    _ => None,
                };

                if let Some(e) = raw_event {
                    let _ = tx.send(e);
                }
            };

            if let Err(e) = listen(callback) {
                tracing::error!("Keyboard listener error: {:?}", e);
            }
        });

        tracing::info!("Keyboard listener started (macOS)");
        Ok(())
    }

    fn stop(&mut self) {
        self.listening.store(false, Ordering::SeqCst);
        tracing::info!("Keyboard listener stopped");
    }

    fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<RawKeyEvent>> {
        self.rx.take()
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::SeqCst)
    }
}

/// Convert rdev Key to our KeyCode
fn convert_key(key: Key) -> KeyCode {
    match key {
        Key::Num0 | Key::Kp0 => KeyCode::Num0,
        Key::Num1 | Key::Kp1 => KeyCode::Num1,
        Key::Num2 | Key::Kp2 => KeyCode::Num2,
        Key::Num3 | Key::Kp3 => KeyCode::Num3,
        Key::Num4 | Key::Kp4 => KeyCode::Num4,
        Key::Num5 | Key::Kp5 => KeyCode::Num5,
        Key::Num6 | Key::Kp6 => KeyCode::Num6,
        Key::Num7 | Key::Kp7 => KeyCode::Num7,
        Key::Num8 | Key::Kp8 => KeyCode::Num8,
        Key::Num9 | Key::Kp9 => KeyCode::Num9,
        Key::KeyA => KeyCode::A,
        Key::KeyB => KeyCode::B,
        Key::KeyC => KeyCode::C,
        Key::KeyD => KeyCode::D,
        Key::KeyE => KeyCode::E,
        Key::KeyF => KeyCode::F,
        Key::KeyG => KeyCode::G,
        Key::KeyH => KeyCode::H,
        Key::KeyI => KeyCode::I,
        Key::KeyJ => KeyCode::J,
        Key::KeyK => KeyCode::K,
        Key::KeyL => KeyCode::L,
        Key::KeyM => KeyCode::M,
        Key::KeyN => KeyCode::N,
        Key::KeyO => KeyCode::O,
        Key::KeyP => KeyCode::P,
        Key::KeyQ => KeyCode::Q,
        Key::KeyR => KeyCode::R,
        Key::KeyS => KeyCode::S,
        Key::KeyT => KeyCode::T,
        Key::KeyU => KeyCode::U,
        Key::KeyV => KeyCode::V,
        Key::KeyW => KeyCode::W,
        Key::KeyX => KeyCode::X,
        Key::KeyY => KeyCode::Y,
        Key::KeyZ => KeyCode::Z,
        Key::Space => KeyCode::Space,
        Key::Return => KeyCode::Enter,
        Key::Escape => KeyCode::Escape,
        Key::Unknown(code) => KeyCode::Other(code as u32),
        _ => KeyCode::Other(0),
    }
}

