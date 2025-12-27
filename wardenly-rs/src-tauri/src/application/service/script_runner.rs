use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::DynamicImage;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::domain::model::{Action, ActionType, LoopConfig, Scene, SceneMatcher, Script, Step};
use crate::infrastructure::browser::BrowserDriver;
use crate::infrastructure::config::resources;

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
    script: Script,
    scenes: Vec<Scene>,
    browser: Arc<dyn BrowserDriver>,
    scene_matcher: SceneMatcher,
    counters: HashMap<String, i32>,
    running: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<ScriptCommand>,
}

impl ScriptRunner {
    pub fn new(
        script: Script,
        scenes: Vec<Scene>,
        browser: Arc<dyn BrowserDriver>,
        cmd_rx: mpsc::Receiver<ScriptCommand>,
    ) -> Self {
        Self {
            script,
            scenes,
            browser,
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

        while self.running.load(Ordering::Relaxed) {
            // Check for stop command
            if let Ok(ScriptCommand::Stop) = self.cmd_rx.try_recv() {
                tracing::info!("Script stop command received");
                return StopReason::Manual;
            }

            // Capture current screen
            let image = match self.browser.capture_screen().await {
                Ok(img) => img,
                Err(e) => {
                    tracing::warn!("Failed to capture screen: {}", e);
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
    async fn execute_step_by_index(&mut self, step_idx: usize, _image: &DynamicImage) -> StepResult {
        if !self.running.load(Ordering::Relaxed) {
            return StepResult::Quit;
        }

        // Clone step data to avoid borrow issues
        let step = self.script.steps[step_idx].clone();

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
        let start_time = Instant::now();

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

            // Safety timeout (5 minutes max)
            if start_time.elapsed() > Duration::from_secs(300) {
                tracing::warn!("Loop timeout reached");
                break;
            }
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
                if action.points.len() >= 2 {
                    let from = action.points[0];
                    let to = action.points[action.points.len() - 1];
                    if let Err(e) = self.browser.drag((from.x, from.y), (to.x, to.y)).await {
                        tracing::warn!("Drag failed: {}", e);
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
                // OCR check would go here
                // For now, just continue
            }
        }

        StepResult::Continue
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

