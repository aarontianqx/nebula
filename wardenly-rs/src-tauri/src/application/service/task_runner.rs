use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::DynamicImage;
use tokio::sync::{broadcast, mpsc};
use tokio::time::sleep;

use super::condition_eval;
use super::humanize;
use super::script_runner::{ScriptCommand, StopReason};
use crate::application::eventbus::SharedEventBus;
use crate::domain::event::DomainEvent;
use crate::domain::model::{
    FieldCondition, MatchPredicate, NoMatchPolicy, OcrAction, OcrMode, OcrRule, Point, QuitReason,
    Scene, SceneMatcher, SharedGameState, Task, TaskAction, TaskStep,
};
use crate::infrastructure::browser::{BrowserDriver, BrowserPoint};
use crate::infrastructure::config::resources::{self, ProtocolRegistry};
use crate::infrastructure::ocr::{OcrClientHandle, Roi};

/// Default timeout for wait_protocol / wait_state actions.
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
/// Interval between match evaluations while waiting in on_no_match=wait.
const NO_MATCH_POLL: Duration = Duration::from_millis(500);

/// TaskRunner is the single, task-agnostic executor (see
/// specs/proposals/unified-task-runner.md). It runs a state-matching loop:
/// every iteration executes the first step (template order = priority) whose
/// predicate holds and whose `once` marker is not yet consumed. Linear flows
/// are the special case where every step is `once`; looping flows (battle
/// rounds) are the general case where a predicate stays true across
/// iterations.
///
/// All task knowledge lives in the template: predicates (scene and/or
/// state./role. conditions), actions (protocol and screenshot fallbacks
/// mixed freely), thresholds, and the no-match policy.
pub struct TaskRunner {
    session_id: String,
    task: Task,
    scenes: Vec<Scene>,
    browser: Arc<dyn BrowserDriver>,
    ocr_client: OcrClientHandle,
    event_bus: SharedEventBus,
    game_state: SharedGameState,
    registry: Option<ProtocolRegistry>,
    scene_matcher: SceneMatcher,
    counters: HashMap<String, i32>,
    /// Indices of `once` steps already executed in this run.
    once_done: HashSet<usize>,
    running: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<ScriptCommand>,
}

/// Outcome of a single wait-style action.
enum WaitOutcome {
    Matched,
    Timeout,
    /// An `abort_if` condition took hold — the wait's premise is gone
    /// (e.g. battle over); hand back to the state machine immediately.
    Aborted,
    Stopped,
}

impl TaskRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        task: Task,
        scenes: Vec<Scene>,
        browser: Arc<dyn BrowserDriver>,
        ocr_client: OcrClientHandle,
        event_bus: SharedEventBus,
        game_state: SharedGameState,
        registry: Option<ProtocolRegistry>,
        cmd_rx: mpsc::Receiver<ScriptCommand>,
    ) -> Self {
        Self {
            session_id,
            task,
            scenes,
            browser,
            ocr_client,
            event_bus,
            game_state,
            registry,
            scene_matcher: SceneMatcher::default(),
            counters: HashMap::new(),
            once_done: HashSet::new(),
            running: Arc::new(AtomicBool::new(true)),
            cmd_rx,
        }
    }

    /// Replace the internal running flag with an externally shared one.
    pub fn set_running_flag(&mut self, running: Arc<AtomicBool>) {
        self.running = running;
    }

    /// Main execution loop.
    pub async fn run(&mut self) -> StopReason {
        tracing::info!(session = %self.session_id, task = %self.task.name, "Task started");

        if let Err(reason) = self.validate_protocols() {
            return reason;
        }

        // Subscribe BEFORE any protocol action so responses can't race past us.
        let mut events = self.event_bus.subscribe();
        let mut no_match_since: Option<Instant> = None;

        loop {
            if self.stop_requested() {
                return StopReason::Manual;
            }

            match self.find_matching_step().await {
                Some(idx) => {
                    no_match_since = None;
                    // Clone what we need so the loop can borrow self mutably.
                    let step = self.task.steps[idx].clone();
                    tracing::info!(session = %self.session_id, task = %self.task.name, step = %step.name, "Step matched");
                    self.event_bus.publish(DomainEvent::ScriptStepExecuted {
                        session_id: self.session_id.clone(),
                        step_index: idx,
                        scene_name: step.name.clone(),
                    });

                    // Rules first (same decision point as scene scripts).
                    if let Some(result) = self.check_rules(&step).await {
                        return result;
                    }

                    if let Err(reason) = self
                        .execute_actions(&step.actions, &step.name, &mut events)
                        .await
                    {
                        // A quit action (completed/exhausted) or manual stop is
                        // a normal ending, not a failure — don't log ERROR.
                        match reason {
                            StopReason::Error => {
                                tracing::error!(session = %self.session_id, task = %self.task.name, step = %step.name, "Step failed: {:?}", reason)
                            }
                            _ => {
                                tracing::info!(session = %self.session_id, task = %self.task.name, step = %step.name, "Step ended: {:?}", reason)
                            }
                        }
                        return reason;
                    }

                    if step.match_.once {
                        self.once_done.insert(idx);
                    }
                }
                None => match self.task.on_no_match.policy {
                    NoMatchPolicy::Quit => {
                        tracing::info!(session = %self.session_id, task = %self.task.name, "No step matches, task completed");
                        return StopReason::Completed;
                    }
                    NoMatchPolicy::Wait => {
                        let since = no_match_since.get_or_insert_with(Instant::now);
                        let timeout = self
                            .task
                            .on_no_match
                            .timeout
                            .unwrap_or(Duration::from_secs(120));
                        if since.elapsed() > timeout {
                            tracing::info!(
                                session = %self.session_id,
                                task = %self.task.name,
                                "No step matched within {:?}, task completed",
                                timeout
                            );
                            return StopReason::Completed;
                        }
                        tokio::select! {
                            _ = sleep(NO_MATCH_POLL) => {}
                            _ = self.cmd_rx.recv() => return StopReason::Manual,
                        }
                    }
                },
            }
        }
    }

    /// Find the first step (template order) whose predicate holds.
    async fn find_matching_step(&mut self) -> Option<usize> {
        // One screenshot per evaluation round, only if some step needs it.
        let needs_scene = self.task.steps.iter().enumerate().any(|(i, s)| {
            s.match_.scene.is_some() && !(s.match_.once && self.once_done.contains(&i))
        });
        let screen: Option<DynamicImage> = if needs_scene {
            self.browser.capture_screen().await.ok()
        } else {
            None
        };

        for (idx, step) in self.task.steps.iter().enumerate() {
            if step.match_.once && self.once_done.contains(&idx) {
                continue;
            }
            if self.predicate_holds(&step.match_, screen.as_ref()).await {
                return Some(idx);
            }
        }
        None
    }

    async fn predicate_holds(
        &self,
        predicate: &MatchPredicate,
        screen: Option<&DynamicImage>,
    ) -> bool {
        if let Some(scene_name) = &predicate.scene {
            let matched = match screen {
                Some(image) => resources::find_scene(&self.scenes, scene_name)
                    .map(|scene| self.scene_matcher.matches(scene, image))
                    .unwrap_or(false),
                None => false,
            };
            if !matched {
                return false;
            }
        }
        condition_eval::conditions_met(
            &predicate.conditions,
            &self.game_state,
            &self.browser,
            false,
        )
        .await
    }

    /// Evaluate step rules (stateRule / ocrRule); Some(reason) ends the task.
    async fn check_rules(&mut self, step: &TaskStep) -> Option<StopReason> {
        if let Some(state_rule) = &step.state_rule {
            let met = condition_eval::conditions_met(
                &state_rule.conditions,
                &self.game_state,
                &self.browser,
                state_rule.any,
            )
            .await;
            tracing::info!(
                step = %step.name,
                condition_met = met,
                action = ?state_rule.action,
                "State rule evaluated"
            );
            if met {
                match state_rule.action {
                    OcrAction::QuitExhausted => return Some(StopReason::ResourceExhausted),
                    OcrAction::Quit => return Some(StopReason::Completed),
                    OcrAction::Skip => {}
                }
            }
        }

        if let Some(ocr_rule) = &step.ocr_rule {
            if let Some(reason) = self.check_ocr_rule(ocr_rule, &step.name).await {
                return Some(reason);
            }
        }
        None
    }

    /// OCR fallback rule check (captures a fresh screenshot).
    async fn check_ocr_rule(&self, rule: &OcrRule, step_name: &str) -> Option<StopReason> {
        if !self.ocr_client.is_healthy() {
            tracing::debug!(step = %step_name, "OCR service unavailable, skipping rule");
            return None;
        }
        let image = self.browser.capture_screen().await.ok()?;
        match rule.mode {
            OcrMode::Ratio => {
                let roi = Roi {
                    x: rule.roi.x,
                    y: rule.roi.y,
                    width: rule.roi.width,
                    height: rule.roi.height,
                };
                let result = self
                    .ocr_client
                    .recognize_usage_ratio(&image, Some(&roi))
                    .await
                    .ok()?;
                let mut ctx = crate::domain::model::ExprContext::new();
                ctx.set("used", result.denominator as i64);
                ctx.set("total", result.numerator as i64);
                let met = ctx.evaluate(&rule.condition).unwrap_or(false);
                tracing::info!(step = %step_name, condition = %rule.condition, condition_met = met, "OCR rule evaluated");
                if !met {
                    return None;
                }
                match rule.action {
                    OcrAction::QuitExhausted => Some(StopReason::ResourceExhausted),
                    OcrAction::Quit => Some(StopReason::Completed),
                    OcrAction::Skip => None,
                }
            }
        }
    }

    /// Execute a sequence of actions (used for steps and loop bodies).
    async fn execute_actions(
        &mut self,
        actions: &[TaskAction],
        step_name: &str,
        events: &mut broadcast::Receiver<DomainEvent>,
    ) -> Result<(), StopReason> {
        for action in actions {
            if self.stop_requested() {
                return Err(StopReason::Manual);
            }
            humanize::pace().await;

            match action {
                TaskAction::Click { points } => {
                    for point in points {
                        if let Err(e) = self.browser.click(point.x, point.y).await {
                            tracing::warn!(step = %step_name, "Click failed: {}", e);
                            return Err(StopReason::Error);
                        }
                    }
                }
                TaskAction::Drag { points } => {
                    self.execute_drag(points, step_name).await?;
                }
                TaskAction::Wait { duration } => {
                    let d = duration.unwrap_or(Duration::from_secs(1));
                    tokio::select! {
                        _ = sleep(d) => {}
                        _ = self.cmd_rx.recv() => return Err(StopReason::Manual),
                    }
                }
                TaskAction::Loop {
                    count,
                    interval,
                    until,
                    actions,
                } => {
                    self.execute_loop(
                        *count,
                        *interval,
                        until.as_ref(),
                        actions,
                        step_name,
                        events,
                    )
                    .await?;
                }
                TaskAction::Incr { key } => {
                    *self.counters.entry(key.clone()).or_insert(0) += 1;
                }
                TaskAction::Decr { key } => {
                    *self.counters.entry(key.clone()).or_insert(0) -= 1;
                }
                TaskAction::Quit { condition, reason } => {
                    let should_quit = match condition {
                        Some(cond) => cond.evaluate(&self.counters),
                        None => true,
                    };
                    if should_quit {
                        return Err(match reason {
                            Some(QuitReason::Exhausted) => StopReason::ResourceExhausted,
                            _ => StopReason::Completed,
                        });
                    }
                }
                TaskAction::SendProtocol { protocol, payload } => {
                    self.send_protocol(protocol, payload, step_name).await?;
                }
                TaskAction::Request {
                    protocol,
                    payload,
                    expect,
                    expect_any,
                    timeout,
                    conditions,
                    retries,
                    on_timeout,
                    abort_if,
                } => {
                    let expects: Vec<String> = expect
                        .iter()
                        .cloned()
                        .chain(expect_any.iter().cloned())
                        .collect();
                    if expects.is_empty() {
                        tracing::error!(step = %step_name, "Request {} has neither expect nor expect_any", protocol);
                        return Err(StopReason::Error);
                    }
                    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT);
                    let expect_refs: Vec<&str> = expects.iter().map(String::as_str).collect();
                    // Resolve `$`-refs in response conditions once (e.g. own
                    // role name for self-attack confirmation) — per-message
                    // resolution would eval the bridge per broadcast.
                    let resolved_conditions = condition_eval::resolve_condition_refs(
                        conditions,
                        &self.game_state,
                        &self.browser,
                    )
                    .await;
                    // Resolve $-refs once per action execution. If the
                    // selection basis is gone (e.g. the team list just went
                    // empty), skip the whole action: waiting for a response
                    // to a request we never sent would be a fake timeout.
                    let Some(resolved_payload) = condition_eval::resolve_payload_refs(
                        payload,
                        &self.game_state,
                        &self.browser,
                    )
                    .await
                    else {
                        tracing::warn!(
                            session = %self.session_id,
                            step = %step_name,
                            "Skipping {}: payload $-references unresolved (selection basis gone)",
                            protocol
                        );
                        continue;
                    };
                    // Drain buffered events before the first send: a request's
                    // response must post-date the request. Messages that pile
                    // up between waits (e.g. the PREVIOUS battle's RESULT)
                    // must never answer a new wait.
                    while events.try_recv().is_ok() {}
                    let mut attempt = 0u32;
                    loop {
                        // The request's premise may have expired between step
                        // match and send, or between attempts (e.g. the battle
                        // ended while earlier actions ran) — never send into
                        // an aborted state; hand back to the state machine.
                        if !abort_if.is_empty()
                            && condition_eval::conditions_met(
                                abort_if,
                                &self.game_state,
                                &self.browser,
                                false,
                            )
                            .await
                        {
                            tracing::info!(
                                session = %self.session_id,
                                step = %step_name,
                                "Request {} aborted by abort_if before send",
                                protocol
                            );
                            break;
                        }
                        self.send_protocol(protocol, &resolved_payload, step_name)
                            .await?;
                        match self
                            .wait_protocol(
                                &expect_refs,
                                &resolved_conditions,
                                timeout,
                                abort_if,
                                events,
                            )
                            .await
                        {
                            WaitOutcome::Matched => break,
                            WaitOutcome::Aborted => {
                                tracing::info!(
                                    session = %self.session_id,
                                    step = %step_name,
                                    "Request {} wait aborted by abort_if",
                                    protocol
                                );
                                break;
                            }
                            WaitOutcome::Stopped => return Err(StopReason::Manual),
                            WaitOutcome::Timeout => {
                                attempt += 1;
                                if attempt > *retries {
                                    if matches!(
                                        on_timeout,
                                        crate::domain::model::OnTimeout::Continue
                                    ) {
                                        tracing::warn!(
                                            session = %self.session_id,
                                            step = %step_name,
                                            "Request {} -> {:?} timed out after {} attempt(s), continuing",
                                            protocol,
                                            expects,
                                            attempt
                                        );
                                        break;
                                    }
                                    tracing::error!(
                                        session = %self.session_id,
                                        step = %step_name,
                                        "Request {} -> {:?} timed out after {} attempt(s)",
                                        protocol,
                                        expects,
                                        attempt
                                    );
                                    return Err(StopReason::Error);
                                }
                                tracing::warn!(
                                    session = %self.session_id,
                                    step = %step_name,
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
                TaskAction::WaitProtocol {
                    protocol,
                    timeout,
                    conditions,
                } => {
                    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT);
                    let resolved_conditions = condition_eval::resolve_condition_refs(
                        conditions,
                        &self.game_state,
                        &self.browser,
                    )
                    .await;
                    match self
                        .wait_protocol(
                            &[protocol.as_str()],
                            &resolved_conditions,
                            timeout,
                            &[],
                            events,
                        )
                        .await
                    {
                        WaitOutcome::Matched | WaitOutcome::Aborted => {}
                        WaitOutcome::Timeout => {
                            tracing::error!(step = %step_name, "Timeout ({:?}) waiting for protocol {}", timeout, protocol);
                            return Err(StopReason::Error);
                        }
                        WaitOutcome::Stopped => return Err(StopReason::Manual),
                    }
                }
                TaskAction::WaitState {
                    timeout,
                    conditions,
                } => {
                    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT);
                    match self.wait_state(conditions, timeout).await {
                        // wait_state has no abort_if input; Aborted is unreachable.
                        WaitOutcome::Matched | WaitOutcome::Aborted => {}
                        WaitOutcome::Timeout => {
                            tracing::error!(step = %step_name, "Timeout ({:?}) waiting for game state", timeout);
                            return Err(StopReason::Error);
                        }
                        WaitOutcome::Stopped => return Err(StopReason::Manual),
                    }
                }
                TaskAction::EvalJs { script } => {
                    // Wrap so the result is always a string: evaluate() fails
                    // on undefined returns, and JS errors must surface loudly.
                    let Ok(script_literal) = serde_json::to_string(script) else {
                        return Err(StopReason::Error);
                    };
                    let wrapped = format!(
                        "(() => {{ try {{ const r = eval({}); return r === undefined ? 'OK' : String(r); }} catch (e) {{ return 'ERR ' + (e && e.message ? e.message : String(e)); }} }})()",
                        script_literal
                    );
                    match self.browser.evaluate(&wrapped).await {
                        Ok(result) if !result.contains("ERR") => {
                            tracing::debug!(step = %step_name, "eval_js -> {}", result)
                        }
                        Ok(result) => {
                            tracing::error!(step = %step_name, "eval_js error: {}", result);
                            return Err(StopReason::Error);
                        }
                        Err(e) => {
                            tracing::error!(step = %step_name, "eval_js failed: {}", e);
                            return Err(StopReason::Error);
                        }
                    }
                }
                TaskAction::LogState { paths } => {
                    for path in paths {
                        // Best-effort observability: unresolved paths log as such
                        // (that fact is itself diagnostic).
                        match condition_eval::resolve_path_pub(
                            path,
                            &self.game_state,
                            &self.browser,
                        )
                        .await
                        {
                            Some(value) => {
                                tracing::info!(session = %self.session_id, step = %step_name, "state {} = {}", path, value)
                            }
                            None => {
                                tracing::info!(session = %self.session_id, step = %step_name, "state {} = <unresolved>", path)
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Execute a loop action (single level, like the legacy runner).
    async fn execute_loop(
        &mut self,
        count: i32,
        interval: Option<Duration>,
        until: Option<&String>,
        actions: &[TaskAction],
        step_name: &str,
        events: &mut broadcast::Receiver<DomainEvent>,
    ) -> Result<(), StopReason> {
        let is_infinite = count < 0;
        let mut iteration = 0usize;

        while !self.stop_requested() {
            for action in actions {
                // Nested loops are not supported; guard at runtime.
                if matches!(action, TaskAction::Loop { .. }) {
                    tracing::warn!(step = %step_name, "Nested loops are not supported, skipping");
                    continue;
                }
                // Reuse the flat action executor for the single action.
                // Box::pin breaks the execute_actions ↔ execute_loop async
                // recursion (nested loops are rejected above at runtime).
                Box::pin(self.execute_actions(std::slice::from_ref(action), step_name, events))
                    .await?;
            }

            // until: exit when the scene matches
            if let Some(until_scene) = until {
                if let Ok(image) = self.browser.capture_screen().await {
                    if let Some(scene) = resources::find_scene(&self.scenes, until_scene) {
                        if self.scene_matcher.matches(scene, &image) {
                            tracing::debug!(step = %step_name, scene = %until_scene, "Loop until-scene matched");
                            break;
                        }
                    }
                }
            }

            iteration += 1;
            if !is_infinite && iteration >= count as usize {
                break;
            }
            sleep(interval.unwrap_or(Duration::from_millis(300))).await;
        }
        Ok(())
    }

    async fn execute_drag(&self, points: &[Point], step_name: &str) -> Result<(), StopReason> {
        if points.len() < 2 {
            return Ok(());
        }
        let browser_points: Vec<BrowserPoint> =
            points.iter().map(|p| BrowserPoint::new(p.x, p.y)).collect();
        if let Err(e) = self.browser.drag_path(&browser_points).await {
            tracing::warn!(step = %step_name, "Drag failed: {}", e);
            return Err(StopReason::Error);
        }
        Ok(())
    }

    /// Send a protocol message after resolving `$` references in the payload.
    async fn send_protocol(
        &self,
        protocol: &str,
        payload: &serde_json::Value,
        step_name: &str,
    ) -> Result<(), StopReason> {
        let Some(payload) =
            condition_eval::resolve_payload_refs(payload, &self.game_state, &self.browser).await
        else {
            // A $-ref that no longer resolves is a race, not an error: the
            // selection basis vanished mid-step (e.g. the team list just went
            // empty). Skip the action and let the state machine re-evaluate
            // with the fresh data.
            tracing::warn!(
                step = %step_name,
                "Skipping {}: payload $-references unresolved (selection basis gone)",
                protocol
            );
            return Ok(());
        };

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
    /// payload satisfies all conditions. While waiting, `abort_if` conditions
    /// are polled on a short tick: when they hold, the wait ends as Aborted
    /// (the wait's premise is gone — e.g. the battle ended mid-wait).
    async fn wait_protocol(
        &mut self,
        protocols: &[&str],
        conditions: &[FieldCondition],
        timeout: Duration,
        abort_if: &[FieldCondition],
        events: &mut broadcast::Receiver<DomainEvent>,
    ) -> WaitOutcome {
        const ABORT_POLL: Duration = Duration::from_millis(200);
        let deadline = Instant::now() + timeout;

        if !abort_if.is_empty()
            && condition_eval::conditions_met(abort_if, &self.game_state, &self.browser, false)
                .await
        {
            return WaitOutcome::Aborted;
        }

        loop {
            if self.stop_requested() {
                return WaitOutcome::Stopped;
            }
            let now = Instant::now();
            if now >= deadline {
                return WaitOutcome::Timeout;
            }

            tokio::select! {
                _ = self.cmd_rx.recv() => return WaitOutcome::Stopped,
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
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(session = %self.session_id, "Protocol event stream lagged by {} messages", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => return WaitOutcome::Stopped,
                    }
                }
                // Tick: re-check the deadline and poll abort conditions.
                // Abort polling lives only here (not per event) so a message
                // flood can't turn into a CDP-eval storm.
                _ = sleep((deadline - now).min(ABORT_POLL)) => {
                    if !abort_if.is_empty()
                        && condition_eval::conditions_met(
                            abort_if,
                            &self.game_state,
                            &self.browser,
                            false,
                        )
                        .await
                    {
                        return WaitOutcome::Aborted;
                    }
                }
            }
        }
    }

    /// Wait until all state./role. conditions hold (readiness gate).
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

            let remaining = deadline - now;
            tokio::select! {
                _ = sleep(remaining.min(Duration::from_millis(200))) => {}
                _ = self.cmd_rx.recv() => return WaitOutcome::Stopped,
            }
        }
    }

    /// Validate referenced protocol names against the registry (when loaded).
    fn validate_protocols(&self) -> Result<(), StopReason> {
        let Some(registry) = &self.registry else {
            return Ok(());
        };

        let mut unknown = Vec::new();
        let mut check = |name: &str| {
            if !registry.contains(name) {
                unknown.push(name.to_string());
            }
        };
        fn collect(actions: &[TaskAction], check: &mut impl FnMut(&str)) {
            for action in actions {
                match action {
                    TaskAction::SendProtocol { protocol, .. } => check(protocol),
                    TaskAction::Request {
                        protocol,
                        expect,
                        expect_any,
                        ..
                    } => {
                        check(protocol);
                        expect.iter().for_each(|e| check(e));
                        expect_any.iter().for_each(|e| check(e));
                    }
                    TaskAction::WaitProtocol { protocol, .. } => check(protocol),
                    TaskAction::Loop { actions, .. } => collect(actions, check),
                    _ => {}
                }
            }
        }
        for step in &self.task.steps {
            collect(&step.actions, &mut check);
        }

        if unknown.is_empty() {
            Ok(())
        } else {
            unknown.sort();
            unknown.dedup();
            tracing::error!(
                task = %self.task.name,
                "Unknown protocol names (registry bundle {}): {}",
                registry.bundle_version,
                unknown.join(", ")
            );
            Err(StopReason::Error)
        }
    }

    fn stop_requested(&self) -> bool {
        // The shared flag is always set before the Stop command is sent
        // (ScriptHandle::stop), so flag-only checking loses no immediacy.
        !self.running.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::eventbus::create_event_bus;
    use crate::domain::model::{
        new_shared_game_state, ColorPoint, ColorValue, MatchPredicate, NoMatchRule, Scene, TaskStep,
    };
    use crate::infrastructure::ocr::global_ocr_client;
    use serde_json::json;

    /// Mock driver: solid-color screen, canned queryRole(7) wire response,
    /// no-op click.
    struct MockDriver;

    #[async_trait::async_trait]
    impl BrowserDriver for MockDriver {
        async fn start(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn stop(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn navigate(&self, _url: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn click(&self, _x: f64, _y: f64) -> anyhow::Result<()> {
            Ok(())
        }
        async fn drag(&self, _from: (f64, f64), _to: (f64, f64)) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn drag_path(&self, _points: &[BrowserPoint]) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn start_screencast(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn stop_screencast(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn evaluate(&self, _script: &str) -> anyhow::Result<String> {
            Ok(serde_json::to_string(&json!({"ok": true, "value": 7}).to_string()).unwrap())
        }
        async fn install_page_bridge(
            &self,
            _binding_name: &str,
            _init_script: &str,
        ) -> anyhow::Result<mpsc::Receiver<String>> {
            unimplemented!()
        }
        async fn capture_screen(&self) -> anyhow::Result<DynamicImage> {
            Ok(DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
                1080,
                720,
                image::Rgba([33, 0, 0, 255]),
            )))
        }
        async fn input_text(&self, _selector: &str, _text: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn click_element(&self, _selector: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn wait_visible(&self, _selector: &str, _timeout: Duration) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn login_with_password(
            &self,
            _username: &str,
            _password: &str,
            _timeout: Duration,
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn refresh(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn insert_text(&self, _text: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    fn step(name: &str, predicate: MatchPredicate, actions: Vec<TaskAction>) -> TaskStep {
        TaskStep {
            name: name.to_string(),
            match_: predicate,
            state_rule: None,
            ocr_rule: None,
            actions,
        }
    }

    fn once_step(name: &str, actions: Vec<TaskAction>) -> TaskStep {
        step(
            name,
            MatchPredicate {
                scene: None,
                conditions: vec![],
                once: true,
            },
            actions,
        )
    }

    fn build_runner(task: Task) -> (TaskRunner, mpsc::Sender<ScriptCommand>) {
        // The sender must outlive the run: a closed command channel is
        // interpreted as a stop signal.
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let runner = TaskRunner::new(
            "s".to_string(),
            task,
            vec![],
            Arc::new(MockDriver),
            global_ocr_client(),
            create_event_bus(),
            new_shared_game_state(),
            None,
            cmd_rx,
        );
        (runner, cmd_tx)
    }

    /// Linear flow: all-once steps run in order exactly once, then no match
    /// ends the task as Completed.
    #[tokio::test(flavor = "multi_thread")]
    async fn linear_once_flow_completes() {
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![
                once_step(
                    "first",
                    vec![TaskAction::Incr {
                        key: "a".to_string(),
                    }],
                ),
                once_step(
                    "second",
                    vec![TaskAction::Quit {
                        condition: None,
                        reason: None,
                    }],
                ),
            ],
        };
        let (mut runner, _cmd_tx) = build_runner(task);
        let reason = tokio::time::timeout(Duration::from_secs(15), runner.run())
            .await
            .expect("linear flow hung (once semantics broken?)");
        assert_eq!(reason, StopReason::Completed);
    }

    /// Priority: a state-condition finish step short-circuits everything.
    #[tokio::test(flavor = "multi_thread")]
    async fn finish_on_state_condition() {
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![step(
                "finish",
                MatchPredicate {
                    scene: None,
                    conditions: vec![FieldCondition {
                        field: "role._knightTower._teamNumInfo.num".to_string(),
                        op: "gte".to_string(),
                        value: json!(7),
                    }],
                    once: false,
                },
                vec![TaskAction::Quit {
                    condition: None,
                    reason: Some(QuitReason::Exhausted),
                }],
            )],
        };
        let (mut runner, _cmd_tx) = build_runner(task);
        let reason = runner.run().await;
        assert_eq!(reason, StopReason::ResourceExhausted);
    }

    /// Mixed: scene predicate + click fallback + quit in one step.
    #[tokio::test(flavor = "multi_thread")]
    async fn scene_predicate_with_click_fallback() {
        let scene = Scene {
            name: "solid".to_string(),
            category: String::new(),
            points: vec![ColorPoint {
                x: 0,
                y: 0,
                color: ColorValue {
                    r: 33,
                    g: 0,
                    b: 0,
                    a: Some(255),
                },
            }],
            actions: Default::default(),
        };
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![step(
                "popup",
                MatchPredicate {
                    scene: Some("solid".to_string()),
                    conditions: vec![],
                    once: true,
                },
                vec![
                    TaskAction::Click {
                        points: vec![Point { x: 1.0, y: 2.0 }],
                    },
                    TaskAction::Quit {
                        condition: None,
                        reason: None,
                    },
                ],
            )],
        };
        let (_cmd_tx, cmd_rx) = mpsc::channel(8);
        let mut runner = TaskRunner::new(
            "s".to_string(),
            task,
            vec![scene],
            Arc::new(MockDriver),
            global_ocr_client(),
            create_event_bus(),
            new_shared_game_state(),
            None,
            cmd_rx,
        );
        let reason = runner.run().await;
        assert_eq!(reason, StopReason::Completed);
    }

    /// on_no_match=wait respects its timeout and completes.
    #[tokio::test(flavor = "multi_thread")]
    async fn wait_policy_times_out_as_completed() {
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule {
                policy: NoMatchPolicy::Wait,
                timeout: Some(Duration::from_secs(2)),
            },
            steps: vec![step(
                "never",
                MatchPredicate {
                    scene: None,
                    conditions: vec![FieldCondition {
                        field: "role.nothing.here".to_string(),
                        op: "exists".to_string(),
                        value: serde_json::Value::Null,
                    }],
                    once: false,
                },
                vec![],
            )],
        };
        // role.* resolves via the mock to 7 for any path... except paths the
        // mock can't see. The mock returns 7 for everything, so "exists"
        // would match! Give a state.* path instead (never populated).
        let mut task = task;
        task.steps[0].match_.conditions[0].field = "state.NOPE.x".to_string();

        let (mut runner, _cmd_tx) = build_runner(task);
        let reason = runner.run().await;
        assert_eq!(reason, StopReason::Completed);
    }

    /// A request's response must post-date its send: stale buffered events
    /// (e.g. the previous battle's RESULT) must never answer a new wait.
    /// The runner drains the receiver before the first send; without draining,
    /// this wait would match the stale RESULT instantly instead of timing out.
    #[tokio::test(flavor = "multi_thread")]
    async fn request_drains_stale_events_before_send() {
        let event_bus = create_event_bus();
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![once_step(
                "attack",
                vec![TaskAction::Request {
                    protocol: "C_2_S_TEST".to_string(),
                    payload: json!({}),
                    expect: None,
                    expect_any: vec!["S_2_C_KNIGHT_TOWER_RESULT".to_string()],
                    timeout: Some(Duration::from_secs(1)),
                    conditions: vec![],
                    retries: 0,
                    on_timeout: crate::domain::model::OnTimeout::Continue,
                    abort_if: vec![],
                }],
            )],
        };

        let (_cmd_tx2, cmd_rx) = mpsc::channel(8);
        let mut runner = TaskRunner::new(
            "s".to_string(),
            task,
            vec![],
            Arc::new(MockDriver),
            global_ocr_client(),
            event_bus.clone(),
            new_shared_game_state(),
            None,
            cmd_rx,
        );

        // A RESULT lands before the request's send (pacing buys us the window)
        // and must be drained, not matched.
        let bus = event_bus.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            bus.publish(DomainEvent::ProtocolMessage {
                session_id: "s".to_string(),
                protocol_id: 3118,
                name: Some("S_2_C_KNIGHT_TOWER_RESULT".to_string()),
                data: json!({}),
            });
        });

        let start = std::time::Instant::now();
        let reason = runner.run().await;
        let elapsed = start.elapsed();

        assert_eq!(reason, StopReason::Completed);
        assert!(
            elapsed >= Duration::from_millis(900),
            "stale RESULT answered the wait (drain broken), elapsed={elapsed:?}"
        );
    }

    /// An unresolvable payload $-reference (selection basis vanished mid-step,
    /// e.g. the team list went empty) skips the action instead of failing the
    /// task — the state machine re-evaluates with fresh data.
    #[tokio::test(flavor = "multi_thread")]
    async fn unresolvable_payload_ref_skips_action() {
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![once_step(
                "join",
                vec![TaskAction::SendProtocol {
                    protocol: "C_2_S_TEST".to_string(),
                    payload: json!({
                        "create_id": "$state.NOPE.missing.path",
                    }),
                }],
            )],
        };
        let (mut runner, _cmd_tx) = build_runner(task);
        let reason = runner.run().await;
        assert_eq!(reason, StopReason::Completed);
    }

    /// The default on_timeout policy is Continue: a request that times out
    /// (without an explicit `on_timeout: fail`) must not kill the task.
    #[tokio::test(flavor = "multi_thread")]
    async fn request_timeout_defaults_to_continue() {
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![once_step(
                "req",
                vec![TaskAction::Request {
                    protocol: "C_2_S_TEST".to_string(),
                    payload: json!({}),
                    expect: Some("S_2_C_NEVER_ARRIVES".to_string()),
                    expect_any: vec![],
                    timeout: Some(Duration::from_millis(300)),
                    conditions: vec![],
                    retries: 0,
                    on_timeout: Default::default(),
                    abort_if: vec![],
                }],
            )],
        };
        let (mut runner, _cmd_tx) = build_runner(task);
        let reason = runner.run().await;
        assert_eq!(reason, StopReason::Completed);
    }

    /// abort_if aborts an in-flight wait the moment its premise vanishes
    /// (e.g. battle RESULT resets isBattle mid-wait): no full timeout burn,
    /// and the task hands back to the state machine (Continue semantics).
    #[tokio::test(flavor = "multi_thread")]
    async fn request_aborts_when_abort_if_holds_mid_wait() {
        let game_state = new_shared_game_state();
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![once_step(
                "attack",
                vec![TaskAction::Request {
                    protocol: "C_2_S_TEST".to_string(),
                    payload: json!({}),
                    expect: Some("S_2_C_NEVER_ARRIVES".to_string()),
                    expect_any: vec![],
                    timeout: Some(Duration::from_secs(5)),
                    conditions: vec![],
                    retries: 3,
                    on_timeout: crate::domain::model::OnTimeout::Continue,
                    abort_if: vec![FieldCondition {
                        field: "state.S_2_C_BATTLE_OVER".to_string(),
                        op: "exists".to_string(),
                        value: json!(null),
                    }],
                }],
            )],
        };
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let mut runner = TaskRunner::new(
            "s".to_string(),
            task,
            vec![],
            Arc::new(MockDriver),
            global_ocr_client(),
            create_event_bus(),
            game_state.clone(),
            None,
            cmd_rx,
        );
        let _cmd_tx = cmd_tx; // keep the command channel open

        // The premise vanishes 300ms into the wait.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            game_state
                .write()
                .unwrap()
                .update("S_2_C_BATTLE_OVER", json!({}));
        });

        let start = std::time::Instant::now();
        let reason = runner.run().await;
        let elapsed = start.elapsed();

        assert_eq!(reason, StopReason::Completed);
        assert!(
            elapsed < Duration::from_secs(2),
            "abort_if did not short-circuit the wait, elapsed={elapsed:?}"
        );
    }

    /// Request conditions support `$`-refs resolved once per action (e.g.
    /// comparing a broadcast's `name` against our own role name). A teammate's
    /// PLAYER_ATTACK must NOT confirm our attack wait; our own must.
    #[tokio::test(flavor = "multi_thread")]
    async fn request_condition_matches_only_own_broadcast() {
        let event_bus = create_event_bus();
        let task = Task {
            name: "t".to_string(),
            description: None,
            on_no_match: NoMatchRule::default(),
            steps: vec![once_step(
                "attack",
                vec![TaskAction::Request {
                    protocol: "C_2_S_TEST".to_string(),
                    payload: json!({}),
                    expect: Some("S_2_C_PLAYER_ATTACK".to_string()),
                    expect_any: vec![],
                    timeout: Some(Duration::from_secs(5)),
                    conditions: vec![FieldCondition {
                        field: "name".to_string(),
                        op: "eq".to_string(),
                        value: json!("$role.accName"),
                    }],
                    retries: 0,
                    on_timeout: Default::default(),
                    abort_if: vec![],
                }],
            )],
        };
        let (_cmd_tx, cmd_rx) = mpsc::channel(8);
        let mut runner = TaskRunner::new(
            "s".to_string(),
            task,
            vec![],
            Arc::new(MockDriver), // queryRole returns 7 → our "name" is 7
            global_ocr_client(),
            event_bus.clone(),
            new_shared_game_state(),
            None,
            cmd_rx,
        );

        let bus = event_bus.clone();
        tokio::spawn(async move {
            let publish = |name: serde_json::Value| {
                bus.publish(DomainEvent::ProtocolMessage {
                    session_id: "s".to_string(),
                    protocol_id: 1,
                    name: Some("S_2_C_PLAYER_ATTACK".to_string()),
                    data: json!({ "name": name }),
                });
            };
            // A teammate's attack lands while we wait — must not match.
            tokio::time::sleep(Duration::from_millis(800)).await;
            publish(json!("teammate"));
            // Ours lands later — matches (name == resolved 7).
            tokio::time::sleep(Duration::from_millis(800)).await;
            publish(json!(7));
        });

        let start = std::time::Instant::now();
        let reason = runner.run().await;
        let elapsed = start.elapsed();

        assert_eq!(reason, StopReason::Completed);
        assert!(
            elapsed >= Duration::from_millis(1400),
            "teammate's broadcast confirmed our wait (elapsed={elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_secs(4),
            "own broadcast did not confirm the wait (elapsed={elapsed:?})"
        );
    }
}
