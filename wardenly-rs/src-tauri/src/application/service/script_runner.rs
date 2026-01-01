use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use image::DynamicImage;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::application::eventbus::SharedEventBus;
use crate::domain::event::DomainEvent;
use crate::domain::model::{Action, ActionType, LoopConfig, OcrRule, Scene, SceneMatcher, Script, Step};
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

    /// Main execution loop
    pub async fn run(&mut self) -> StopReason {
        tracing::info!(script = %self.script.name, "Script started");

        let default_wait = Duration::from_millis(500);
        let mut capture_failures: u32 = 0;

        while self.running.load(Ordering::Relaxed) {
            // Check for stop command
            if let Ok(ScriptCommand::Stop) = self.cmd_rx.try_recv() {
                tracing::info!("Script stop command received");
                return StopReason::Manual;
            }

            // Capture current screen
            let image = match self.browser.capture_screen().await {
                Ok(img) => {
                    capture_failures = 0; // Reset on success
                    img
                }
                Err(e) => {
                    capture_failures += 1;
                    tracing::warn!(attempt = capture_failures, "Failed to capture screen: {}", e);
                    
                    if capture_failures >= MAX_CAPTURE_FAILURES {
                        tracing::error!("Too many capture failures, browser may have stopped");
                        return StopReason::BrowserStopped;
                    }
                    
                    if self.running.load(Ordering::Relaxed) {
                        sleep(default_wait).await;
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

            // Wait before next iteration
            if self.running.load(Ordering::Relaxed) {
                sleep(default_wait).await;
            }
        }

        StopReason::Manual
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
            if let Some(result) = self.check_ocr_rule(ocr_rule, image).await {
                return result;
            }
        }

        // Handle looped actions
        if let Some(loop_config) = &step.loop_config {
            return self.execute_loop_cloned(&step, loop_config.clone()).await;
        }

        // Execute actions sequentially
        self.execute_actions(&step.actions).await
    }

    /// Execute actions in a loop (with cloned data)
    async fn execute_loop_cloned(&mut self, step: &Step, loop_config: LoopConfig) -> StepResult {
        // Validate loop indices
        if let Err(e) = loop_config.validate_indices(step.actions.len()) {
            tracing::error!("Invalid loop configuration: {}", e);
            return StepResult::Error;
        }

        let start_idx = loop_config.start_index;
        let end_idx = loop_config.end_index;

        // Execute pre-loop actions
        if start_idx > 0 {
            let result = self.execute_actions(&step.actions[..start_idx]).await;
            if result != StepResult::Continue {
                return result;
            }
        }

        // Execute loop
        let mut iteration = 0;

        while self.running.load(Ordering::Relaxed) {
            // Check for stop command
            if let Ok(ScriptCommand::Stop) = self.cmd_rx.try_recv() {
                return StepResult::Quit;
            }

            // Execute loop actions
            let loop_actions = &step.actions[start_idx..=end_idx];
            let result = self.execute_actions(loop_actions).await;
            if result != StepResult::Continue {
                return result;
            }

            // Check until condition
            if let Some(until_scene) = &loop_config.until {
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
            if !loop_config.is_infinite() && iteration >= loop_config.count as usize {
                break;
            }

            // Loop interval
            if let Some(interval) = loop_config.interval {
                sleep(interval).await;
            } else {
                sleep(Duration::from_millis(300)).await;
            }

            // (5-minute safety timeout removed per user request - loops can run indefinitely)
        }

        // Execute post-loop actions
        if end_idx + 1 < step.actions.len() {
            return self.execute_actions(&step.actions[end_idx + 1..]).await;
        }

        StepResult::Continue
    }

    /// Execute a list of actions
    async fn execute_actions(&mut self, actions: &[Action]) -> StepResult {
        for action in actions {
            if !self.running.load(Ordering::Relaxed) {
                return StepResult::Quit;
            }

            let result = self.execute_action(action).await;
            if result != StepResult::Continue {
                return result;
            }
        }
        StepResult::Continue
    }

    /// Execute a single action
    async fn execute_action(&mut self, action: &Action) -> StepResult {
        match action.action_type {
            ActionType::Click => {
                if let Some(point) = action.points.first() {
                    if let Err(e) = self.browser.click(point.x, point.y).await {
                        tracing::warn!("Click failed: {}", e);
                    }
                }
            }

            ActionType::Wait => {
                if let Some(duration) = action.duration {
                    sleep(duration).await;
                }
            }

            ActionType::Drag => {
                if action.points.len() == 2 {
                    // Two-point drag: use interpolated drag
                    let from = action.points[0];
                    let to = action.points[1];
                    if let Err(e) = self.browser.drag((from.x, from.y), (to.x, to.y)).await {
                        tracing::warn!("Drag failed: {}", e);
                    }
                } else if action.points.len() > 2 {
                    // Multi-point path: use drag_path for precise path following
                    let browser_points: Vec<BrowserPoint> = action
                        .points
                        .iter()
                        .map(|p| BrowserPoint::new(p.x, p.y))
                        .collect();
                    if let Err(e) = self.browser.drag_path(&browser_points).await {
                        tracing::warn!("Drag path failed: {}", e);
                    }
                }
            }

            ActionType::Incr => {
                if let Some(key) = &action.key {
                    *self.counters.entry(key.clone()).or_insert(0) += 1;
                }
            }

            ActionType::Decr => {
                if let Some(key) = &action.key {
                    *self.counters.entry(key.clone()).or_insert(0) -= 1;
                }
            }

            ActionType::Quit => {
                if let Some(condition) = &action.condition {
                    if condition.evaluate(&self.counters) {
                        return StepResult::Quit;
                    }
                } else {
                    return StepResult::Quit;
                }
            }

            ActionType::CheckScene => {
                // OCR check handled at step level in execute_step_by_index
            }
        }

        StepResult::Continue
    }

    /// Check OCR rule and return StepResult if should stop
    async fn check_ocr_rule(&self, ocr_rule: &OcrRule, image: &DynamicImage) -> Option<StepResult> {
        // Only handle "quit_when_exhausted" rule
        if ocr_rule.name != "quit_when_exhausted" {
            tracing::warn!(rule = %ocr_rule.name, "Unknown OCR rule, skipping");
            return None;
        }

        // Check if OCR service is healthy
        if !self.ocr_client.is_healthy() {
            tracing::debug!("OCR service unavailable, skipping rule check");
            return None;
        }

        // Perform OCR recognition
        let roi = Roi {
            x: ocr_rule.roi.x,
            y: ocr_rule.roi.y,
            width: ocr_rule.roi.width,
            height: ocr_rule.roi.height,
        };

        match self.ocr_client.recognize_usage_ratio(image, Some(&roi)).await {
            Ok(result) => {
                tracing::info!(
                    rule = %ocr_rule.name,
                    numerator = result.numerator,
                    denominator = result.denominator,
                    threshold = ocr_rule.threshold,
                    "OCR result"
                );

                // quit_when_exhausted: stop if denominator exceeds threshold or denominator > numerator
                if result.denominator > ocr_rule.threshold || result.denominator > result.numerator {
                    tracing::info!("Resource exhausted detected, stopping script");
                    return Some(StepResult::ResourceExhausted);
                }
            }
            Err(e) => {
                tracing::warn!("OCR recognition failed: {}", e);
                // Don't stop on OCR failure - continue execution
            }
        }

        None
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
}

impl ScriptHandle {
    /// Send stop command to the script
    pub async fn stop(&self) {
        let _ = self.cmd_tx.send(ScriptCommand::Stop).await;
    }
}

