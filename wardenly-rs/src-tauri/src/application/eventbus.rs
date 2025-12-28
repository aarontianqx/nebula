use crate::domain::event::DomainEvent;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Event bus for publishing and subscribing to domain events
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: DomainEvent) {
        if let Err(e) = self.sender.send(event) {
            tracing::trace!("No subscribers for event: {}", e);
        }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

/// Shared event bus wrapped in Arc for thread-safe sharing
pub type SharedEventBus = Arc<EventBus>;

pub fn create_event_bus() -> SharedEventBus {
    Arc::new(EventBus::default())
}

