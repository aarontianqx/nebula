use std::time::Instant;
use tokio::sync::mpsc;

/// Key codes for keyboard events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Space,
    Enter,
    Escape,
    Other(u32),
}

/// Type of keyboard event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    Press,
    Release,
}

/// Raw keyboard event from the system
#[derive(Debug, Clone)]
pub struct RawKeyEvent {
    pub key: KeyCode,
    pub event_type: KeyEventType,
    pub timestamp: Instant,
}

/// Trait for platform-specific keyboard listeners
pub trait KeyboardListener: Send + Sync {
    /// Start listening for keyboard events
    fn start(&mut self) -> anyhow::Result<()>;

    /// Stop listening for keyboard events
    fn stop(&mut self);

    /// Take the receiver channel for keyboard events
    fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<RawKeyEvent>>;

    /// Check if currently listening
    fn is_listening(&self) -> bool;
}

