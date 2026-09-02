use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc};
use tokio::time::sleep;

use super::condition_eval;
use super::script_runner::{ScriptCommand, StopReason};
use crate::application::eventbus::SharedEventBus;
use crate::domain::event::DomainEvent;
use crate::domain::model::{
    FieldCondition, ProtocolAction, ProtocolScript, ProtocolStep, SharedGameState,
};
use crate::infrastructure::browser::BrowserDriver;
use crate::infrastructure::config::resources::ProtocolRegistry;

/// Default timeout for wait_protocol actions.
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// ProtocolRunner executes protocol-driven scripts: a linear pass through the
/// steps, sending and observing game protocols instead of matching scenes.
///
/// Contrast with ScriptRunner (scene loop): here there is no screenshot/OCR in
/// the execution path at all — waits block on the structured protocol event
/// stream and conditions read decoded fields from the shared GameState.
/// Click/drag actions exist only as a fallback for UI unreachable by protocol.
pub struct ProtocolRunner {
    session_id: String,
    script: ProtocolScript,
    browser: Arc<dyn BrowserDriver>,
    event_bus: SharedEventBus,
    game_state: SharedGameState,
    registry: Option<ProtocolRegistry>,
    running: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<ScriptCommand>,
}

/// Why a wait_protocol action ended.
enum WaitOutcome {
    Matched,
    Timeout,
    Stopped,
}

impl ProtocolRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        script: ProtocolScript,
        browser: Arc<dyn BrowserDriver>,
        event_bus: SharedEventBus,
        game_state: SharedGameState,
        registry: Option<ProtocolRegistry>,
        cmd_rx: mpsc::Receiver<ScriptCommand>,
    ) -> Self {
        Self {
            session_id,
            script,
            browser,
            event_bus,
            game_state,
            registry,
            running: Arc::new(AtomicBool::new(true)),
            cmd_rx,
        }
    }

    /// Replace the internal running flag with an externally shared one.
    pub fn set_running_flag(&mut self, running: Arc<AtomicBool>) {
        self.running = running;
    }

    /// Execute the script straight through, once.
    pub async fn run(&mut self) -> StopReason {
        tracing::info!(script = %self.script.name, "Protocol script started");

        if let Err(reason) = self.validate_protocols() {
            return reason;
        }

        // Subscribe BEFORE any send so responses can never race past us.
        let mut events = self.event_bus.subscribe();

        for (index, step) in self.script.steps.clone().iter().enumerate() {
            if self.stop_requested() {
                return StopReason::Manual;
            }

            if !self.step_conditions_met(step).await {
                tracing::info!(
                    script = %self.script.name,
                    step = %step.name,
                    "Step skipped: conditions not met"
                );
                continue;
            }

            tracing::info!(script = %self.script.name, step = %step.name, "Step started");
            match self.execute_step(step, &mut events).await {
                Ok(()) => {
                    self.event_bus.publish(DomainEvent::ScriptStepExecuted {
                        session_id: self.session_id.clone(),
                        step_index: index,
                        scene_name: step.name.clone(),
                    });
                }
                Err(reason) => return reason,
            }
        }

        tracing::info!(script = %self.script.name, "Protocol script completed");
        StopReason::Completed
    }

    /// Validate all referenced protocol names against the registry (when loaded).
    /// The bridge also resolves names in-page, but failing fast with a clear
    /// message beats a script that dies mid-run on a typo.
    fn validate_protocols(&self) -> Result<(), StopReason> {
        let Some(registry) = &self.registry else {
            return Ok(());
        };

        let mut unknown = Vec::new();
        for step in &self.script.steps {
            for action in &step.actions {
                match action {
                    ProtocolAction::SendProtocol { protocol, .. }
                    | ProtocolAction::WaitProtocol { protocol, .. } => {
                        if !registry.contains(protocol) {
                            unknown.push(protocol.clone());
                        }
                    }
                    ProtocolAction::Request {
                        protocol,
                        expect,
                        expect_any,
                        ..
                    } => {
                        let names = expect
                            .iter()
                            .chain(expect_any.iter())
                            .chain(std::iter::once(protocol));
                        for name in names {
                            if !registry.contains(name) {
                                unknown.push(name.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if unknown.is_empty() {
            Ok(())
        } else {
            tracing::error!(
                script = %self.script.name,
                "Unknown protocol names (registry bundle {}): {}",
                registry.bundle_version,
                unknown.join(", ")
            );
            Err(StopReason::Error)
        }
    }

    async fn step_conditions_met(&self, step: &ProtocolStep) -> bool {
        condition_eval::conditions_met(&step.conditions, &self.game_state, &self.browser, false)
            .await
    }

    /// Wait until all conditions hold (readiness gate). Paths may use `state.`
    /// (latest pushed payload) or `role.` (client role model, queried live).
    async fn wait_state(
        &mut self,
        conditions: &[FieldCondition],
        timeout: Duration,
    ) -> WaitOutcome {
        let deadline = Instant::now() + timeout;

        loop {
            if self.stop_requested() {
                return WaitOutcome::Stopped;
            }
            if condition_eval::conditions_met(conditions, &self.game_state, &self.browser, false)
                .await
            {
                return WaitOutcome::Matched;
            }
            let now = Instant::now();
            if now >= deadline {
                return WaitOutcome::Timeout;
            }

            // Poll on a short interval; state updates are cheap to re-check and
            // this needs no coupling to which protocol produced the change.
            let remaining = deadline - now;
            tokio::select! {
                _ = sleep(remaining.min(Duration::from_millis(200))) => {}
                _ = self.cmd_rx.recv() => {
                    return WaitOutcome::Stopped;
                }
            }
        }
    }

    async fn execute_step(
        &mut self,
        step: &ProtocolStep,
        events: &mut broadcast::Receiver<DomainEvent>,
    ) -> Result<(), StopReason> {
        for action in &step.actions {
            if self.stop_requested() {
                return Err(StopReason::Manual);
            }

            match action {
                ProtocolAction::SendProtocol { protocol, payload } => {
                    self.send_protocol(protocol, payload).await?;
                }
                ProtocolAction::Request {
                    protocol,
                    payload,
                    expect,
                    expect_any,
                    timeout,
                    conditions,
                    retries,
                } => {
                    let expects: Vec<&str> = expect
                        .iter()
                        .map(String::as_str)
                        .chain(expect_any.iter().map(String::as_str))
                        .collect();
                    if expects.is_empty() {
                        tracing::error!(
                            step = %step.name,
                            "Request {} has neither expect nor expect_any",
                            protocol
                        );
                        return Err(StopReason::Error);
                    }
                    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT);
                    let mut attempt = 0u32;
                    loop {
                        self.send_protocol(protocol, payload).await?;
                        match self
                            .wait_protocol(&expects, conditions, timeout, events)
                            .await
                        {
                            WaitOutcome::Matched => break,
                            WaitOutcome::Stopped => return Err(StopReason::Manual),
                            WaitOutcome::Timeout => {
                                attempt += 1;
                                if attempt > *retries {
                                    tracing::error!(
                                        step = %step.name,
                                        "Request {} -> {:?} timed out after {} attempt(s)",
                                        protocol,
                                        expects,
                                        attempt
                                    );
                                    return Err(StopReason::Error);
                                }
                                tracing::warn!(
                                    step = %step.name,
                                    "Request {} -> {:?} timed out, retrying ({}/{})",
                                    protocol,
                                    expects,
                                    attempt,
                                    retries
                                );
                            }
                        }
                    }
                }
                ProtocolAction::WaitProtocol {
                    protocol,
                    timeout,
                    conditions,
                } => {
                    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT);
                    match self
                        .wait_protocol(&[protocol.as_str()], conditions, timeout, events)
                        .await
                    {
                        WaitOutcome::Matched => {}
                        WaitOutcome::Timeout => {
                            tracing::error!(
                                step = %step.name,
                                "Timeout ({:?}) waiting for protocol {}",
                                timeout,
                                protocol
                            );
                            return Err(StopReason::Error);
                        }
                        WaitOutcome::Stopped => return Err(StopReason::Manual),
                    }
                }
                ProtocolAction::WaitState {
                    timeout,
                    conditions,
                } => {
                    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT);
                    match self.wait_state(conditions, timeout).await {
                        WaitOutcome::Matched => {}
                        WaitOutcome::Timeout => {
                            tracing::error!(
                                step = %step.name,
                                "Timeout ({:?}) waiting for game state",
                                timeout
                            );
                            return Err(StopReason::Error);
                        }
                        WaitOutcome::Stopped => return Err(StopReason::Manual),
                    }
                }
                ProtocolAction::Wait { duration } => {
                    self.interruptible_wait(duration.unwrap_or(Duration::from_secs(1)))
                        .await?;
                }
                ProtocolAction::Click { points } => {
                    for point in points {
                        if let Err(e) = self.browser.click(point.x, point.y).await {
                            tracing::warn!(step = %step.name, "Click failed: {}", e);
                            return Err(StopReason::Error);
                        }
                    }
                }
                ProtocolAction::Drag { points } => {
                    if let (Some(first), Some(last)) = (points.first(), points.last()) {
                        if let Err(e) = self
                            .browser
                            .drag((first.x, first.y), (last.x, last.y))
                            .await
                        {
                            tracing::warn!(step = %step.name, "Drag failed: {}", e);
                            return Err(StopReason::Error);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Send a protocol message through the page bridge (same mechanism as
    /// SessionCommand::SendProtocol, without leaving the runner task).
    async fn send_protocol(
        &self,
        protocol: &str,
        payload: &serde_json::Value,
    ) -> Result<(), StopReason> {
        let name_literal = match serde_json::to_string(protocol) {
            Ok(lit) => lit,
            Err(e) => {
                tracing::error!("Failed to serialize protocol name: {}", e);
                return Err(StopReason::Error);
            }
        };
        let script = format!(
            "window.__wardenly ? window.__wardenly.send({}, {}) : 'ERR bridge not installed'",
            name_literal, payload
        );

        match self.browser.evaluate(&script).await {
            Ok(result) if !result.contains("ERR") => {
                tracing::debug!("send_protocol {} -> {}", protocol, result);
                Ok(())
            }
            Ok(result) => {
                tracing::error!("bridge rejected send {}: {}", protocol, result);
                Err(StopReason::Error)
            }
            Err(e) => {
                tracing::error!("evaluate failed for send {}: {}", protocol, e);
                Err(StopReason::Error)
            }
        }
    }

    /// Wait for a downstream message of any of the given protocols whose
    /// payload satisfies all conditions.
    async fn wait_protocol(
        &mut self,
        protocols: &[&str],
        conditions: &[FieldCondition],
        timeout: Duration,
        events: &mut broadcast::Receiver<DomainEvent>,
    ) -> WaitOutcome {
        let deadline = Instant::now() + timeout;

        loop {
            if self.stop_requested() {
                return WaitOutcome::Stopped;
            }
            let now = Instant::now();
            if now >= deadline {
                return WaitOutcome::Timeout;
            }

            tokio::select! {
                // Stop command (or channel close) interrupts the wait.
                _ = self.cmd_rx.recv() => {
                    return WaitOutcome::Stopped;
                }
                ev = events.recv() => {
                    match ev {
                        Ok(DomainEvent::ProtocolMessage {
                            session_id,
                            name: Some(name),
                            data,
                            ..
                        }) if session_id == self.session_id && protocols.contains(&name.as_str()) => {
                            if conditions.iter().all(|c| c.evaluate(&data)) {
                                tracing::debug!("wait_protocol {:?} matched by {}", protocols, name);
                                return WaitOutcome::Matched;
                            }
                            // Right protocol but conditions unmet: keep waiting.
                            tracing::debug!(
                                "wait_protocol {:?}: {} arrived but conditions unmet",
                                protocols, name
                            );
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Protocol event stream lagged by {} messages", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            return WaitOutcome::Stopped;
                        }
                    }
                }
                _ = sleep(deadline - now) => {
                    return WaitOutcome::Timeout;
                }
            }
        }
    }

    /// Sleep, unless a stop arrives first.
    async fn interruptible_wait(&mut self, duration: Duration) -> Result<(), StopReason> {
        tokio::select! {
            _ = sleep(duration) => Ok(()),
            _ = self.cmd_rx.recv() => Err(StopReason::Manual),
        }
    }

    fn stop_requested(&self) -> bool {
        // The shared flag is always set before the Stop command is sent
        // (ScriptHandle::stop), so flag-only checking loses no immediacy.
        !self.running.load(Ordering::Relaxed)
    }
}
