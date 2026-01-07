use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use image::DynamicImage;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::application::eventbus::SharedEventBus;
use crate::domain::event::DomainEvent;
use crate::domain::model::{
    Action, ExprContext, OcrAction, OcrMode, OcrRule, Point, Scene, SceneMatcher, Script,
};
use crate::infrastructure::browser::{BrowserDriver, BrowserPoint};
use crate::infrastructure::config::resources;
use crate::infrastructure::ocr::{OcrClientHandle, Roi};

/// Command to control script execution
#[derive(Debug)]
pub enum ScriptCommand {
    Stop,
}

/// Result of script execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Manual,
    Completed,
    ResourceExhausted,
    BrowserStopped,
    Error,
}

/// Result of step execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepResult {
    Continue,
    Quit,
    ResourceExhausted,
    Error,
}

/// ScriptRunner executes automation scripts
pub struct ScriptRunner {
    session_id: String,
    script: Script,
    scenes: Vec<Scene>,
    browser: Arc<dyn BrowserDriver>,
    ocr_client: OcrClientHandle,
    event_bus: SharedEventBus,
    scene_matcher: SceneMatcher,
    counters: HashMap<String, i32>,
    running: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<ScriptCommand>,
}

/// Maximum consecutive capture failures before stopping
const MAX_CAPTURE_FAILURES: u32 = 10;

impl ScriptRunner {
    pub fn new(
        session_id: String,
        script: Script,
        scenes: Vec<Scene>,
        browser: Arc<dyn BrowserDriver>,
        ocr_client: OcrClientHandle,
        event_bus: SharedEventBus,
        cmd_rx: mpsc::Receiver<ScriptCommand>,
    ) -> Self {
        Self {
            session_id,
            script,
            scenes,
            browser,
            ocr_client,
            event_bus,
            scene_matcher: SceneMatcher::default(),
            counters: HashMap::new(),
            running: Arc::new(AtomicBool::new(true)),
            cmd_rx,
        }
    }

    /// Replace the internal running flag with an externally shared one.
    /// This allows the ScriptHandle to directly signal stop to the runner.
    pub fn set_running_flag(&mut self, running: Arc<AtomicBool>) {
        self.running = running;
    }

    /// Main execution loop
    pub async fn run(&mut self) -> StopReason {
        tracing::info!(script = %self.script.name, "Script started");

        let default_wait = Duration::from_millis(500);
        let mut capture_failures: u32 = 0;

        loop {
            // Check for stop command (non-blocking)
            match self.cmd_rx.try_recv() {
                Ok(ScriptCommand::Stop) => {
                    tracing::info!("Script stop command received");
                    return StopReason::Manual;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed, exit gracefully
                    tracing::info!("Script command channel closed");
                    return StopReason::Manual;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No command, continue
                }
            }

            // Capture current screen
            let image = match self.browser.capture_screen().await {
                Ok(img) => {
                    capture_failures = 0;
                    img
                }
                Err(e) => {
                    capture_failures += 1;
                    tracing::warn!(attempt = capture_failures, "Failed to capture screen: {}", e);

                    if capture_failures >= MAX_CAPTURE_FAILURES {
                        tracing::error!("Too many capture failures, browser may have stopped");
                        return StopReason::BrowserStopped;
                    }

                    // Wait before retry, but allow stop command to interrupt
                    tokio::select! {
                        _ = sleep(default_wait) => {}
                        _ = self.cmd_rx.recv() => {
                            tracing::info!("Script stop command received during retry wait");
                            return StopReason::Manual;
                        }
                    }
                    continue;
                }
            };

            // Try to find matching scene index
            let matched_step_idx = self.find_matching_step(&image);

            if let Some(step_idx) = matched_step_idx {
                let scene_name = self.script.steps[step_idx].expected_scene.clone();
                tracing::debug!(scene = %scene_name, "Scene matched");

                // Execute matched step
                let result = self.execute_step_by_index(step_idx, &image).await;
                match result {
                    StepResult::Quit => {
                        tracing::info!(script = %self.script.name, "Script completed");
                        return StopReason::Completed;
                    }
                    StepResult::ResourceExhausted => {
                        tracing::info!(script = %self.script.name, "Script stopped: resource exhausted");
                        return StopReason::ResourceExhausted;
                    }
                    StepResult::Error => {
                        tracing::error!(script = %self.script.name, "Script error");
                        return StopReason::Error;
                    }
                    StepResult::Continue => {}
                }
            }

            // Wait before next iteration, but allow stop command to interrupt
            tokio::select! {
                _ = sleep(default_wait) => {}
                _ = self.cmd_rx.recv() => {
                    tracing::info!("Script stop command received during wait");
                    return StopReason::Manual;
                }
            }
        }
    }

    /// Find the index of the first matching step
    fn find_matching_step(&self, image: &DynamicImage) -> Option<usize> {
        for (idx, step) in self.script.steps.iter().enumerate() {
            if let Some(scene) = resources::find_scene(&self.scenes, &step.expected_scene) {
                if self.scene_matcher.matches(scene, image) {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// Execute a step by index
    async fn execute_step_by_index(&mut self, step_idx: usize, image: &DynamicImage) -> StepResult {
        if !self.running.load(Ordering::Relaxed) {
            return StepResult::Quit;
        }

        // Clone step data to avoid borrow issues
        let step = self.script.steps[step_idx].clone();
        let scene_name = step.expected_scene.clone();

        // Publish step executed event for progress tracking
        self.event_bus.publish(DomainEvent::ScriptStepExecuted {
            session_id: self.session_id.clone(),
            step_index: step_idx,
            scene_name: scene_name.clone(),
        });

        // Check OCR rule before executing actions
        if let Some(ref ocr_rule) = step.ocr_rule {
            if let Some(result) = self.check_ocr_rule(ocr_rule, image, &scene_name).await {
                return result;
            }
        }

        // Execute actions sequentially, passing OCR rule for loop iteration checks
        self.execute_actions(&step.actions, step.ocr_rule.as_ref(), &scene_name).await
    }

    /// Execute a list of actions
    async fn execute_actions(
        &mut self,
        actions: &[Action],
        ocr_rule: Option<&OcrRule>,
        scene_name: &str,
    ) -> StepResult {
        for action in actions {
            if !self.running.load(Ordering::Relaxed) {
                return StepResult::Quit;
            }

            let result = self.execute_action(action, ocr_rule, scene_name).await;
            if result != StepResult::Continue {
                return result;
            }
        }
        StepResult::Continue
    }

    /// Execute a single action (now using pattern matching on Action enum)
    async fn execute_action(
        &mut self,
        action: &Action,
        ocr_rule: Option<&OcrRule>,
        scene_name: &str,
    ) -> StepResult {
        match action {
            Action::Click { points } => {
                if let Some(point) = points.first() {
                    if let Err(e) = self.browser.click(point.x, point.y).await {
                        tracing::warn!("Click failed: {}", e);
                    }
                }
            }

            Action::Wait { duration } => {
                if let Some(d) = duration {
                    sleep(*d).await;
                }
            }

            Action::Drag { points } => {
                self.execute_drag(points).await;
            }

            Action::Incr { key } => {
                *self.counters.entry(key.clone()).or_insert(0) += 1;
            }

            Action::Decr { key } => {
                *self.counters.entry(key.clone()).or_insert(0) -= 1;
            }

            Action::Quit { condition } => {
                if let Some(cond) = condition {
                    if cond.evaluate(&self.counters) {
                        return StepResult::Quit;
                    }
                } else {
                    return StepResult::Quit;
                }
            }

            Action::CheckScene => {
                // CheckScene is now handled inside execute_loop for OCR checks
            }

            Action::Loop {
                count,
                interval,
                until,
                actions,
            } => {
                return self
                    .execute_loop(*count, interval.as_ref(), until.as_ref(), actions, ocr_rule, scene_name)
                    .await;
            }
        }

        StepResult::Continue
    }

    /// Execute drag action with points
    async fn execute_drag(&self, points: &[Point]) {
        if points.len() == 2 {
            // Two-point drag: use interpolated drag
            let from = points[0];
            let to = points[1];
            if let Err(e) = self.browser.drag((from.x, from.y), (to.x, to.y)).await {
                tracing::warn!("Drag failed: {}", e);
            }
        } else if points.len() > 2 {
            // Multi-point path: use drag_path for precise path following
            let browser_points: Vec<BrowserPoint> = points
                .iter()
                .map(|p| BrowserPoint::new(p.x, p.y))
                .collect();
            if let Err(e) = self.browser.drag_path(&browser_points).await {
                tracing::warn!("Drag path failed: {}", e);
            }
        }
    }

    /// Execute a loop action with nested actions
    async fn execute_loop(
        &mut self,
        count: i32,
        interval: Option<&Duration>,
        until: Option<&String>,
        actions: &[Action],
        ocr_rule: Option<&OcrRule>,
        scene_name: &str,
    ) -> StepResult {
        let is_infinite = count < 0;
        let mut iteration = 0;

        while self.running.load(Ordering::Relaxed) {
            // Check for stop command
            if let Ok(ScriptCommand::Stop) = self.cmd_rx.try_recv() {
                return StepResult::Quit;
            }

            // Check OCR rule at the start of each iteration (captures fresh screen)
            if let Some(rule) = ocr_rule {
                if let Ok(image) = self.browser.capture_screen().await {
                    if let Some(result) = self.check_ocr_rule(rule, &image, scene_name).await {
                        tracing::info!(iteration, "OCR condition triggered loop exit");
                        return result;
                    }
                }
            }

            // Execute loop body inline (avoiding recursive async call)
            for action in actions {
                if !self.running.load(Ordering::Relaxed) {
                    return StepResult::Quit;
                }

                let result = self.execute_action_non_recursive(action).await;
                if result != StepResult::Continue {
                    return result;
                }
            }

            // Check until condition (scene-based exit)
            if let Some(until_scene) = until {
                if let Ok(image) = self.browser.capture_screen().await {
                    if let Some(scene) = resources::find_scene(&self.scenes, until_scene) {
                        if self.scene_matcher.matches(scene, &image) {
                            tracing::debug!(scene = %until_scene, "Until scene matched, exiting loop");
                            break;
                        }
                    }
                }
            }

            iteration += 1;

            // Check count limit
            if !is_infinite && iteration >= count as usize {
                break;
            }

            // Loop interval
            if let Some(interval_duration) = interval {
                sleep(*interval_duration).await;
            } else {
                sleep(Duration::from_millis(300)).await;
            }
        }

        StepResult::Continue
    }

    /// Execute a single action without Loop support (used inside loops to avoid recursion)
    async fn execute_action_non_recursive(&mut self, action: &Action) -> StepResult {
        match action {
            Action::Click { points } => {
                if let Some(point) = points.first() {
                    if let Err(e) = self.browser.click(point.x, point.y).await {
                        tracing::warn!("Click failed: {}", e);
                    }
                }
            }

            Action::Wait { duration } => {
                if let Some(d) = duration {
                    sleep(*d).await;
                }
            }

            Action::Drag { points } => {
                self.execute_drag(points).await;
            }

            Action::Incr { key } => {
                *self.counters.entry(key.clone()).or_insert(0) += 1;
            }

            Action::Decr { key } => {
                *self.counters.entry(key.clone()).or_insert(0) -= 1;
            }

            Action::Quit { condition } => {
                if let Some(cond) = condition {
                    if cond.evaluate(&self.counters) {
                        return StepResult::Quit;
                    }
                } else {
                    return StepResult::Quit;
                }
            }

            Action::CheckScene => {
                // OCR check handled at step level
            }

            Action::Loop { .. } => {
                // Nested loops not supported (single level only)
                tracing::warn!("Nested loops are not supported, skipping");
            }
        }

        StepResult::Continue
    }

    /// Check OCR rule and return StepResult if condition is met
    async fn check_ocr_rule(
        &self,
        ocr_rule: &OcrRule,
        image: &DynamicImage,
        scene_name: &str,
    ) -> Option<StepResult> {
        // Check if OCR service is healthy
        if !self.ocr_client.is_healthy() {
            tracing::debug!("OCR service unavailable, skipping rule check");
            return None;
        }

        match ocr_rule.mode {
            OcrMode::Ratio => self.check_ocr_ratio_rule(ocr_rule, image, scene_name).await,
        }
    }

    /// Check ratio-based OCR rule using expression evaluation
    async fn check_ocr_ratio_rule(
        &self,
        ocr_rule: &OcrRule,
        image: &DynamicImage,
        scene_name: &str,
    ) -> Option<StepResult> {
        // Perform OCR recognition
        let roi = Roi {
            x: ocr_rule.roi.x,
            y: ocr_rule.roi.y,
            width: ocr_rule.roi.width,
            height: ocr_rule.roi.height,
        };

        match self
            .ocr_client
            .recognize_usage_ratio(image, Some(&roi))
            .await
        {
            Ok(result) => {
                tracing::info!(
                    scene = %scene_name,
                    numerator = result.numerator,
                    denominator = result.denominator,
                    roi = %format!("({},{} {}x{})", roi.x, roi.y, roi.width, roi.height),
                    condition = %ocr_rule.condition,
                    "OCR recognition result"
                );

                // Build expression context with OCR result
                let mut ctx = ExprContext::new();
                ctx.set("used", result.denominator as i64);
                ctx.set("total", result.numerator as i64);

                // Evaluate condition expression
                match ctx.evaluate(&ocr_rule.condition) {
                    Ok(condition_met) => {
                        tracing::info!(
                            scene = %scene_name,
                            condition = %ocr_rule.condition,
                            condition_met,
                            action = ?ocr_rule.action,
                            "OCR condition evaluated"
                        );
                        if condition_met {
                            match ocr_rule.action {
                                OcrAction::QuitExhausted => Some(StepResult::ResourceExhausted),
                                OcrAction::Quit => Some(StepResult::Quit),
                                OcrAction::Skip => None, // Skip means continue to next step
                            }
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            scene = %scene_name,
                            condition = %ocr_rule.condition,
                            error = %e,
                            "OCR condition evaluation failed"
                        );
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!(scene = %scene_name, "OCR recognition failed: {}", e);
                // Don't stop on OCR failure - continue execution
                None
            }
        }
    }

    /// Stop the script runner
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if the script is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// Handle for controlling a running script
pub struct ScriptHandle {
    pub cmd_tx: mpsc::Sender<ScriptCommand>,
    /// Shared flag to signal stop to the runner immediately
    pub running: Arc<AtomicBool>,
    /// Unique identifier for this script run instance
    pub run_id: String,
}

impl ScriptHandle {
    /// Send stop command to the script.
    /// Sets the running flag to false first for immediate effect,
    /// then sends the Stop command through the channel.
    pub async fn stop(&self) {
        // Immediately mark as not running - this allows the runner
        // to detect stop even before processing the channel message
        self.running.store(false, Ordering::Relaxed);
        let _ = self.cmd_tx.send(ScriptCommand::Stop).await;
    }
}
