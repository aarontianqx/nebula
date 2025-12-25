#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod key_click;
mod state;

use key_click::{start_key_click_runner, KeyClickConfig, KeyClickEvent, KeyClickStatus};
use state::{AppState, MousePositionUpdate, PositionPickedEvent, RecordingStatus};
use std::sync::{Arc, Mutex};
use tap_core::{
    delete_profile, export_to_yaml, export_to_yaml_with_metadata, import_from_yaml, list_profiles,
    load_last_used, load_profile, parse_yaml, save_last_used, save_profile, validate_profile,
    Action, ConditionColor, EngineCommand, EngineEvent, EngineState, InjectorExecutor,
    MouseButtonRaw, PlatformConditionProvider, Player, Profile, RawEventType, Recorder,
    RecorderState, Repeat, RunConfig, Timeline, TimedAction, ValidationError, VariableStore,
};
use tap_platform::{
    get_pixel_color, is_window_focused, list_windows, set_dpi_aware, start_input_hook,
    start_mouse_tracker, window_exists, Color, EnigoInjector, InputEventType, MouseButtonType,
    MouseTrackerConfig, MouseTrackerEvent, WindowInfo,
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{debug, error, info, warn};

// === Tauri Commands ===

#[tauri::command]
fn get_default_profile() -> Profile {
    Profile::default()
}

#[tauri::command]
fn get_state(state: State<'_, Mutex<AppState>>) -> EngineState {
    state.lock().unwrap().engine_state
}

#[tauri::command]
fn start_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if app_state.engine_state != EngineState::Idle {
        return Err("Cannot start: not in idle state".into());
    }

    // Send the current profile to the player
    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::SetProfile(app_state.profile.clone()));
        handle.send(EngineCommand::Start);
        info!("Sent start command to player");
    }

    Ok(())
}

#[tauri::command]
fn pause_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if app_state.engine_state != EngineState::Running {
        return Err("Cannot pause: not running".into());
    }

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::Pause);
    }

    Ok(())
}

#[tauri::command]
fn resume_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if app_state.engine_state != EngineState::Paused {
        return Err("Cannot resume: not paused".into());
    }

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::Resume);
    }

    Ok(())
}

#[tauri::command]
fn stop_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::Stop);
    }

    Ok(())
}

#[tauri::command]
fn emergency_stop(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::EmergencyStop);
        warn!("Emergency stop triggered!");
    }

    Ok(())
}

#[tauri::command]
fn update_profile(state: State<'_, Mutex<AppState>>, profile: Profile) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();
    app_state.profile = profile;
    Ok(())
}

#[tauri::command]
fn set_simple_repeat(
    state: State<'_, Mutex<AppState>>,
    action_type: String,
    x: Option<i32>,
    y: Option<i32>,
    key: Option<String>,
    interval_ms: u64,
    repeat_count: Option<u32>,
    countdown_secs: u32,
) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    let action = match action_type.as_str() {
        "click" => Action::Click {
            x: x.unwrap_or(0),
            y: y.unwrap_or(0),
            button: tap_core::MouseButton::Left,
        },
        "key" => Action::KeyTap {
            key: key.unwrap_or_else(|| "Space".into()),
        },
        _ => return Err(format!("Unknown action type: {}", action_type)),
    };

    // Simple repeat: one action followed by a wait
    // The wait is the interval between iterations
    let timeline = Timeline {
        actions: vec![
            TimedAction::after_ms(0, action),
            TimedAction::after_ms(0, Action::Wait { ms: interval_ms }),
        ],
    };

    let repeat = match repeat_count {
        Some(n) => Repeat::Times(n),
        None => Repeat::Forever,
    };

    app_state.profile = Profile {
        name: "Simple Repeat".into(),
        timeline,
        run: RunConfig {
            start_delay_ms: countdown_secs as u64 * 1000,
            speed: 1.0,
            repeat,
        },
        target_window: None,
    };

    info!(?app_state.profile, "Updated profile for simple repeat");

    Ok(())
}

// === Key-to-Click Tool Mode Commands ===

/// Shared injector for key-click mode.
/// We store it separately because AppState already has player which owns the injector.
static KEY_CLICK_INJECTOR: std::sync::OnceLock<Arc<EnigoInjector>> = std::sync::OnceLock::new();

fn get_or_create_injector() -> Arc<EnigoInjector> {
    KEY_CLICK_INJECTOR
        .get_or_init(|| Arc::new(EnigoInjector::new().expect("Failed to create EnigoInjector")))
        .clone()
}

#[tauri::command]
fn start_key_click(
    state: State<'_, Mutex<AppState>>,
    interval_ms: u64,
) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    // Check mutual exclusion: must be idle
    if app_state.engine_state != EngineState::Idle {
        return Err("Cannot start key-click: engine is not idle".into());
    }

    if app_state.recorder.state() != RecorderState::Idle {
        return Err("Cannot start key-click: recording in progress".into());
    }

    if app_state.key_click_handle.is_some() {
        return Err("Key-click mode is already running".into());
    }

    // Get or create the shared injector
    let injector = get_or_create_injector();

    // Start the input hook for capturing key events
    let input_hook = start_input_hook();

    // We need a way to get the current mouse position in the runner.
    // Since we can't easily share the tracker, we'll use rdev to get position.
    // Actually, we can use enigo to get mouse position, but it's simpler to
    // just use the platform API.
    let get_position = move || {
        // Use platform-specific mouse position query
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
            use windows::Win32::Foundation::POINT;
            let mut point = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut point);
            }
            (point.x, point.y)
        }
        #[cfg(target_os = "macos")]
        {
            // On macOS, use Core Graphics
            use core_graphics::event::CGEvent;
            use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
            if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                if let Ok(event) = CGEvent::new(source) {
                    let loc = event.location();
                    return (loc.x as i32, loc.y as i32);
                }
            }
            (0, 0)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            (0, 0)
        }
    };

    let config = KeyClickConfig {
        interval_ms: interval_ms.max(20), // Minimum 20ms interval
    };

    let handle = start_key_click_runner(config, input_hook, injector, get_position);
    app_state.key_click_handle = Some(handle);

    info!(interval_ms, "Key-click mode started");

    Ok(())
}

#[tauri::command]
fn stop_key_click(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if let Some(handle) = app_state.key_click_handle.take() {
        handle.stop();
        info!("Key-click mode stopped");
    }

    Ok(())
}

#[tauri::command]
fn get_key_click_status(state: State<'_, Mutex<AppState>>) -> KeyClickStatus {
    let app_state = state.lock().unwrap();

    if let Some(ref handle) = app_state.key_click_handle {
        handle.status()
    } else {
        KeyClickStatus {
            running: false,
            click_count: 0,
        }
    }
}

// === Profile Persistence Commands ===

#[tauri::command]
fn cmd_save_profile(state: State<'_, Mutex<AppState>>, name: Option<String>) -> Result<String, String> {
    let mut app_state = state.lock().unwrap();
    
    if let Some(n) = name {
        app_state.profile.name = n;
    }
    
    let path = save_profile(&app_state.profile).map_err(|e| e.to_string())?;
    let _ = save_last_used(&app_state.profile.name);
    
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn cmd_load_profile(state: State<'_, Mutex<AppState>>, name: String) -> Result<Profile, String> {
    let profile = load_profile(&name).map_err(|e| e.to_string())?;
    
    let mut app_state = state.lock().unwrap();
    app_state.profile = profile.clone();
    let _ = save_last_used(&name);
    
    Ok(profile)
}

#[tauri::command]
fn cmd_delete_profile(name: String) -> Result<(), String> {
    delete_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_list_profiles() -> Result<Vec<String>, String> {
    list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_get_last_used() -> Option<String> {
    load_last_used()
}

#[tauri::command]
fn get_current_profile(state: State<'_, Mutex<AppState>>) -> Profile {
    state.lock().unwrap().profile.clone()
}

// === Recording Commands ===

#[tauri::command]
fn start_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Idle {
        return Err("Recording already in progress".into());
    }

    if app_state.engine_state != EngineState::Idle {
        return Err("Cannot record while playing".into());
    }

    // Start the input hook
    let input_hook = start_input_hook();
    app_state.input_hook = Some(input_hook);

    // Start the recorder
    app_state.recorder.start();

    info!("Recording started");
    Ok(())
}

#[tauri::command]
fn pause_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Recording {
        return Err("Not recording".into());
    }

    app_state.recorder.pause();
    info!("Recording paused");
    Ok(())
}

#[tauri::command]
fn resume_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Paused {
        return Err("Not paused".into());
    }

    app_state.recorder.resume();
    info!("Recording resumed");
    Ok(())
}

#[tauri::command]
fn stop_recording(state: State<'_, Mutex<AppState>>) -> Result<Timeline, String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() == RecorderState::Idle {
        return Err("Not recording".into());
    }

    // Stop the input hook
    if let Some(hook) = app_state.input_hook.take() {
        hook.stop();
    }

    // Stop the recorder and get the timeline
    let event = app_state.recorder.stop();
    let timeline = match event {
        Some(tap_core::RecorderEvent::RecordingCompleted { timeline }) => timeline,
        _ => Timeline { actions: vec![] },
    };

    info!("Recording stopped, {} actions captured", timeline.actions.len());

    // Update the profile with the recorded timeline
    app_state.profile.timeline = timeline.clone();
    app_state.profile.name = "Recorded".into();

    Ok(timeline)
}

#[tauri::command]
fn get_recording_status(state: State<'_, Mutex<AppState>>) -> RecordingStatus {
    let app_state = state.lock().unwrap();
    RecordingStatus {
        state: app_state.recorder.state(),
        event_count: app_state.recorder.event_count(),
        duration_ms: app_state.recorder.duration_ms(),
    }
}

// === Global Mouse Position Commands ===

#[tauri::command]
fn start_mouse_tracking(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.mouse_tracker.is_some() {
        return Ok(()); // Already tracking
    }

    let config = MouseTrackerConfig::default();
    let tracker = start_mouse_tracker(config);
    app_state.mouse_tracker = Some(tracker);

    info!("Global mouse tracking started");
    Ok(())
}

#[tauri::command]
fn stop_mouse_tracking(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if let Some(tracker) = app_state.mouse_tracker.take() {
        tracker.stop();
        info!("Global mouse tracking stopped");
    }

    Ok(())
}

// === Picker Window Commands ===

#[tauri::command]
async fn open_picker_window(app: AppHandle) -> Result<(), String> {
    // Check if picker window already exists
    if app.get_webview_window("picker").is_some() {
        info!("Picker window already open");
        return Ok(());
    }

    // Create a new fullscreen transparent overlay window
    let picker_window = WebviewWindowBuilder::new(
        &app,
        "picker",
        WebviewUrl::App("picker.html".into()),
    )
    .title("Pick Position")
    .fullscreen(true)
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .build()
    .map_err(|e| format!("Failed to create picker window: {}", e))?;

    info!("Picker window opened");
    
    // The picker window will handle its own close when position is selected
    let _ = picker_window;

    Ok(())
}

#[tauri::command]
async fn close_picker_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("picker") {
        window.close().map_err(|e| format!("Failed to close picker window: {}", e))?;
        info!("Picker window closed");
    }
    Ok(())
}

#[tauri::command]
async fn picker_position_selected(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    // Close the picker window
    if let Some(window) = app.get_webview_window("picker") {
        let _ = window.close();
    }

    // Convert logical pixels (from browser) to physical pixels (for enigo/rdev)
    // On high DPI screens, the browser's screenX/screenY are in logical pixels,
    // but enigo and rdev work with physical pixels after we set DPI awareness.
    let scale = tap_platform::get_primary_scale_factor();
    let physical_x = (x as f64 * scale).round() as i32;
    let physical_y = (y as f64 * scale).round() as i32;

    info!(
        "Position picked: logical ({}, {}), physical ({}, {}), scale {}",
        x, y, physical_x, physical_y, scale
    );

    // Emit the physical coordinates to the main window
    app.emit("position-picked", PositionPickedEvent { x: physical_x, y: physical_y })
        .map_err(|e| format!("Failed to emit position-picked: {}", e))?;

    Ok(())
}

// === Phase 3: Window and Pixel Commands ===

/// Window info for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfoResponse {
    pub handle: usize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<WindowInfo> for WindowInfoResponse {
    fn from(w: WindowInfo) -> Self {
        Self {
            handle: w.handle,
            title: w.title,
            process_name: w.process_name,
            pid: w.pid,
            x: w.rect.x,
            y: w.rect.y,
            width: w.rect.width,
            height: w.rect.height,
        }
    }
}

/// Color info for frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColorResponse {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}

impl From<Color> for ColorResponse {
    fn from(c: Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            hex: c.to_hex(),
        }
    }
}

#[tauri::command]
fn cmd_list_windows() -> Vec<WindowInfoResponse> {
    list_windows().into_iter().map(|w| w.into()).collect()
}

#[tauri::command]
fn cmd_get_foreground_window() -> Option<WindowInfoResponse> {
    tap_platform::get_foreground_window().map(|w| w.into())
}

#[tauri::command]
fn cmd_get_pixel_color(x: i32, y: i32) -> Option<ColorResponse> {
    get_pixel_color(x, y).map(|c| c.into())
}

#[tauri::command]
fn cmd_check_window_focused(title: Option<String>, process: Option<String>) -> bool {
    is_window_focused(title.as_deref(), process.as_deref())
}

#[tauri::command]
fn cmd_check_window_exists(title: Option<String>, process: Option<String>) -> bool {
    window_exists(title.as_deref(), process.as_deref())
}

// === Phase 4: DSL Commands ===

/// Variable definition for frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariableDefinitionResponse {
    pub name: String,
    pub var_type: String,
    pub default: Option<serde_json::Value>,
    pub description: Option<String>,
}

/// Validation error for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationErrorResponse {
    pub path: String,
    pub message: String,
    pub line: Option<usize>,
}

impl From<ValidationError> for ValidationErrorResponse {
    fn from(e: ValidationError) -> Self {
        Self {
            path: e.path,
            message: e.message,
            line: e.line,
        }
    }
}

#[tauri::command]
fn cmd_export_yaml(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    export_to_yaml(&app_state.profile).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_export_yaml_with_metadata(
    state: State<'_, Mutex<AppState>>,
    description: Option<String>,
    author: Option<String>,
) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    export_to_yaml_with_metadata(&app_state.profile, description, author).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_import_yaml(
    state: State<'_, Mutex<AppState>>,
    yaml_content: String,
) -> Result<Profile, String> {
    // Parse and validate
    let dsl_profile = parse_yaml(&yaml_content).map_err(|e| e.to_string())?;
    
    // Validate
    validate_profile(&dsl_profile).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    
    // Convert to Profile
    let profile = import_from_yaml(&yaml_content).map_err(|e| e.to_string())?;
    
    // Update app state
    let mut app_state = state.lock().unwrap();
    app_state.profile = profile.clone();
    
    Ok(profile)
}

#[tauri::command]
fn cmd_validate_yaml(yaml_content: String) -> Result<(), Vec<ValidationErrorResponse>> {
    let dsl_profile = parse_yaml(&yaml_content).map_err(|e| {
        vec![ValidationErrorResponse {
            path: "".to_string(),
            message: e.to_string(),
            line: None,
        }]
    })?;
    
    validate_profile(&dsl_profile).map_err(|errors| {
        errors.into_iter().map(|e| e.into()).collect()
    })
}

#[tauri::command]
fn cmd_get_macro_variables(state: State<'_, Mutex<AppState>>) -> Vec<VariableDefinitionResponse> {
    let app_state = state.lock().unwrap();
    
    // Export to YAML and parse to get variable definitions
    if let Ok(yaml) = export_to_yaml(&app_state.profile) {
        if let Ok(dsl_profile) = parse_yaml(&yaml) {
            return dsl_profile
                .variables
                .into_iter()
                .map(|(name, def)| VariableDefinitionResponse {
                    name,
                    var_type: match def.var_type {
                        tap_core::VariableType::String => "string".to_string(),
                        tap_core::VariableType::Number => "number".to_string(),
                        tap_core::VariableType::Boolean => "boolean".to_string(),
                    },
                    default: def.default,
                    description: def.description,
                })
                .collect();
        }
    }
    
    Vec::new()
}

#[tauri::command]
fn cmd_set_runtime_variables(
    state: State<'_, Mutex<AppState>>,
    vars: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();
    
    for (key, value) in vars {
        if let Some(s) = value.as_str() {
            app_state.variables.set_variable(&key, s.to_string());
        } else if let Some(n) = value.as_f64() {
            app_state.variables.set_variable(&key, n);
        } else if let Some(b) = value.as_bool() {
            app_state.variables.set_variable(&key, b);
        }
    }
    
    Ok(())
}

#[tauri::command]
fn cmd_get_runtime_variables(
    state: State<'_, Mutex<AppState>>,
) -> std::collections::HashMap<String, serde_json::Value> {
    let app_state = state.lock().unwrap();
    let mut result = std::collections::HashMap::new();
    
    for (key, value) in app_state.variables.all_variables() {
        let json_val = match value {
            tap_core::VariableValue::String(s) => serde_json::Value::String(s.clone()),
            tap_core::VariableValue::Number(n) => {
                serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)))
            }
            tap_core::VariableValue::Boolean(b) => serde_json::Value::Bool(*b),
        };
        result.insert(key.clone(), json_val);
    }
    
    // Also include counters
    for (key, value) in app_state.variables.all_counters() {
        result.insert(key.clone(), serde_json::Value::Number(serde_json::Number::from(value)));
    }
    
    result
}

// === Platform Condition Provider ===

/// Platform condition provider for the engine.
pub struct TauriPlatformProvider;

impl PlatformConditionProvider for TauriPlatformProvider {
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool {
        is_window_focused(title, process)
    }

    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool {
        window_exists(title, process)
    }

    fn get_pixel_color(&self, x: i32, y: i32) -> Option<ConditionColor> {
        get_pixel_color(x, y).map(|c| ConditionColor::new(c.r, c.g, c.b))
    }
}

// === Initialization ===

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tap_tauri=debug,tap_core=debug,tap_platform=debug,tauri=info".into()),
        )
        .try_init();
}

fn setup_app(app: &AppHandle) {
    // Create the injector
    let injector = match EnigoInjector::new() {
        Ok(i) => i,
        Err(e) => {
            error!("Failed to create injector: {:?}", e);
            return;
        }
    };

    // Create the platform condition provider
    let platform_provider = TauriPlatformProvider;

    // Create the player with executor and platform provider
    let executor = InjectorExecutor::new(injector);
    let player_handle = Player::spawn(executor, platform_provider);

    // Create the recorder
    let recorder = Recorder::with_defaults();

    // Start global mouse tracking
    let mouse_tracker = start_mouse_tracker(MouseTrackerConfig::default());

    // Store handles in app state
    let state = AppState {
        engine_state: EngineState::Idle,
        profile: Profile::default(),
        player_handle: Some(player_handle),
        executed_count: 0,
        current_action_index: None,
        recorder,
        input_hook: None,
        mouse_tracker: Some(mouse_tracker),
        variables: VariableStore::new(),
        key_click_handle: None,
    };

    app.manage(Mutex::new(state));

    // Start event polling loop
    let app_handle = app.clone();
    std::thread::spawn(move || {
        poll_events(app_handle);
    });

    info!("App setup complete");
}

fn poll_events(app: AppHandle) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps for smooth recording

        let state: State<'_, Mutex<AppState>> = app.state();

        // Collect player events
        let player_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .player_handle
                .as_ref()
                .map(|h| std::iter::from_fn(|| h.try_recv()).collect())
                .unwrap_or_default()
        };

        // Process player events
        for event in player_events {
            debug!(?event, "received engine event");

            // Update state
            {
                let mut app_state = state.lock().unwrap();
                match &event {
                    EngineEvent::StateChanged { new, .. } => {
                        app_state.engine_state = *new;
                    }
                    EngineEvent::ActionCompleted { index } => {
                        app_state.current_action_index = Some(*index);
                        app_state.executed_count += 1;
                    }
                    EngineEvent::IterationCompleted { iteration } => {
                        debug!(iteration, "iteration completed");
                    }
                    _ => {}
                }
            }

            // Emit to frontend
            if let Err(e) = app.emit("engine-event", &event) {
                warn!("Failed to emit event to frontend: {}", e);
            }
        }

        // Process recording input events
        let input_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .input_hook
                .as_ref()
                .map(|h| h.drain())
                .unwrap_or_default()
        };

        if !input_events.is_empty() {
            let mut app_state = state.lock().unwrap();
            let last_pos = app_state.recorder.last_mouse_position();

            for raw_event in input_events {
                // Convert platform event to core event
                let core_event = convert_input_event(&raw_event.event, last_pos);

                // Push to recorder
                if let Some(recorder_event) = app_state.recorder.push_event(raw_event.timestamp_ms, core_event) {
                    // Emit recording status to frontend
                    if let tap_core::RecorderEvent::EventCaptured { event_count, duration_ms } = recorder_event {
                        let status = RecordingStatus {
                            state: app_state.recorder.state(),
                            event_count,
                            duration_ms,
                        };
                        drop(app_state);
                        if let Err(e) = app.emit("recording-status", &status) {
                            warn!("Failed to emit recording status: {}", e);
                        }
                        app_state = state.lock().unwrap();
                    }
                }
            }
        }

        // Process global mouse tracker events
        let mouse_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .mouse_tracker
                .as_ref()
                .map(|t| t.drain())
                .unwrap_or_default()
        };

        for mouse_event in mouse_events {
            let MouseTrackerEvent::PositionUpdate { x, y } = mouse_event;
            let _ = app.emit("mouse-position", MousePositionUpdate { x, y });
        }

        // Process key-click events
        let key_click_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .key_click_handle
                .as_ref()
                .map(|h| h.drain())
                .unwrap_or_default()
        };

        for event in key_click_events {
            match &event {
                KeyClickEvent::Started => {
                    debug!("Key-click mode started event");
                }
                KeyClickEvent::Click { count, x, y } => {
                    debug!(count, x, y, "Key-click: click performed");
                }
                KeyClickEvent::Stopped { total_clicks } => {
                    debug!(total_clicks, "Key-click mode stopped");
                    // Clean up the handle when stopped
                    let mut app_state = state.lock().unwrap();
                    app_state.key_click_handle = None;
                }
            }

            // Emit to frontend
            if let Err(e) = app.emit("key-click-event", &event) {
                warn!("Failed to emit key-click event: {}", e);
            }
        }
    }
}

/// Convert platform input event to core raw event type.
fn convert_input_event(event: &InputEventType, last_pos: (i32, i32)) -> RawEventType {
    match event {
        InputEventType::MouseMove { x, y } => RawEventType::MouseMove { x: *x, y: *y },
        InputEventType::MouseDown { x, y, button } => {
            let (px, py) = if *x == 0 && *y == 0 { last_pos } else { (*x, *y) };
            RawEventType::MouseDown {
                x: px,
                y: py,
                button: convert_button(*button),
            }
        }
        InputEventType::MouseUp { x, y, button } => {
            let (px, py) = if *x == 0 && *y == 0 { last_pos } else { (*x, *y) };
            RawEventType::MouseUp {
                x: px,
                y: py,
                button: convert_button(*button),
            }
        }
        InputEventType::Scroll { delta_x, delta_y } => RawEventType::Scroll {
            delta_x: *delta_x,
            delta_y: *delta_y,
        },
        InputEventType::KeyDown { key } => RawEventType::KeyDown { key: key.clone() },
        InputEventType::KeyUp { key } => RawEventType::KeyUp { key: key.clone() },
    }
}

fn convert_button(button: MouseButtonType) -> MouseButtonRaw {
    match button {
        MouseButtonType::Left => MouseButtonRaw::Left,
        MouseButtonType::Right => MouseButtonRaw::Right,
        MouseButtonType::Middle => MouseButtonRaw::Middle,
        MouseButtonType::Unknown => MouseButtonRaw::Unknown,
    }
}

/// Handle emergency stop shortcut
fn handle_emergency_stop(app: &AppHandle) {
    warn!("Emergency stop shortcut triggered!");
    let state: State<'_, Mutex<AppState>> = app.state();
    let mut app_state = state.lock().unwrap();

    // Stop player if running
    if let Some(ref player) = app_state.player_handle {
        player.send(EngineCommand::EmergencyStop);
    }

    // Stop key-click mode if running
    if let Some(handle) = app_state.key_click_handle.take() {
        handle.stop();
        info!("Key-click mode stopped by emergency stop");
    }

    drop(app_state);
    if let Err(e) = app.emit("emergency-stop", ()) {
        warn!("Failed to emit emergency-stop: {}", e);
    }
}

fn main() {
    // Set DPI awareness before anything else (Windows)
    set_dpi_aware();

    init_logging();

    // Define emergency stop shortcut: Ctrl+Shift+Backspace
    let emergency_shortcut =
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backspace);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &emergency_shortcut && event.state == ShortcutState::Pressed {
                        handle_emergency_stop(app);
                    }
                })
                .build(),
        )
        .setup(move |app| {
            // Register the shortcut
            if let Err(e) = app.global_shortcut().register(emergency_shortcut.clone()) {
                error!("Failed to register emergency shortcut: {:?}", e);
            } else {
                info!("Emergency stop shortcut registered: Ctrl+Shift+Backspace");
            }

            setup_app(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_profile,
            get_state,
            start_execution,
            pause_execution,
            resume_execution,
            stop_execution,
            emergency_stop,
            update_profile,
            set_simple_repeat,
            cmd_save_profile,
            cmd_load_profile,
            cmd_delete_profile,
            cmd_list_profiles,
            cmd_get_last_used,
            get_current_profile,
            // Recording commands
            start_recording,
            pause_recording,
            resume_recording,
            stop_recording,
            get_recording_status,
            // Global mouse tracking commands
            start_mouse_tracking,
            stop_mouse_tracking,
            // Phase 3: Window and pixel commands
            cmd_list_windows,
            cmd_get_foreground_window,
            cmd_get_pixel_color,
            cmd_check_window_focused,
            cmd_check_window_exists,
            // Picker window commands
            open_picker_window,
            close_picker_window,
            picker_position_selected,
            // Phase 4: DSL commands
            cmd_export_yaml,
            cmd_export_yaml_with_metadata,
            cmd_import_yaml,
            cmd_validate_yaml,
            cmd_get_macro_variables,
            cmd_set_runtime_variables,
            cmd_get_runtime_variables,
            // Key-to-Click tool mode commands
            start_key_click,
            stop_key_click,
            get_key_click_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tap");
}
