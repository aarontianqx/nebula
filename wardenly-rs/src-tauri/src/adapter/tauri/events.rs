use crate::application::eventbus::SharedEventBus;
use crate::domain::event::DomainEvent;
use tauri::{AppHandle, Emitter};

/// Start listening to the event bus and forward events to the frontend
pub fn start_event_forwarder(app: AppHandle, event_bus: SharedEventBus) {
    let mut receiver = event_bus.subscribe();

    // Use tauri's async runtime instead of tokio::spawn directly
    // This ensures the task runs within Tauri's managed runtime
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    forward_event(&app, event);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Event forwarder lagged by {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("Event bus closed, stopping forwarder");
                    break;
                }
            }
        }
    });
}

fn forward_event(app: &AppHandle, event: DomainEvent) {
    let event_name = event.event_name();

    if let Err(e) = app.emit(event_name, &event) {
        tracing::warn!("Failed to emit event {}: {}", event_name, e);
    }
}

