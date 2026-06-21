#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod key_click;
mod state;
mod templates;

use key_click::{start_key_click_runner, KeyClickConfig, KeyClickEvent, KeyClickStatus};
use state::{AppState, MousePositionUpdate, PositionPickedEvent, RecordingStatus};
use std::sync::{Arc, Mutex};
use tap_application::{
    delete_profile, list_profiles, load_document, load_last_used, load_recent, save_document,
    save_last_used, save_recent, ActionExecutor, Coordinator, EngineEvent, EngineState,
    MouseButtonRaw, PlatformConditionProvider, Player, RawEventType, Recorder, RecorderState,
};
use tap_core::{
    document_to_yaml, parse_yaml, validate_profile, Action, ConditionColor, MacroDocument, Profile,
    Repeat, RunConfig, TimedAction, Timeline, ValidationError, VariableValue,
};
use tap_platform::{
    get_pixel_color, is_window_focused, list_windows, set_dpi_aware, start_input_hook,
    start_mouse_tracker, window_exists, Color, EnigoInjector, InputEventType, InputInjector,
    MouseButtonType, MouseTrackerConfig, MouseTrackerEvent, WindowInfo,
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
    state.lock().unwrap().coordinator.engine_state()
}

#[tauri::command]
fn start_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();
    app_state.coordinator.start()?;
    info!("Sent start command to player");
    Ok(())
}

#[tauri::command]
fn pause_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().unwrap().coordinator.pause()
}

#[tauri::command]
fn resume_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().unwrap().coordinator.resume()
}

#[tauri::command]
fn stop_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().unwrap().coordinator.stop();
    Ok(())
}

#[tauri::command]
fn emergency_stop(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    app_state.coordinator.emergency_stop();
    warn!("Emergency stop triggered!");

    // Also stop key-click mode if running (just signal, don't take)
    if let Some(ref handle) = app_state.key_click_handle {
        handle.stop();
        info!("Key-click mode stop requested by emergency stop");
    }

    Ok(())
}

#[tauri::command]
fn update_profile(state: State<'_, Mutex<AppState>>, profile: Profile) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();
    app_state.coordinator.apply_profile_edit(&profile);
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command: arguments mirror the UI form fields.
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

    let profile = Profile {
        name: "Simple Repeat".into(),
        timeline,
        run: RunConfig {
            start_delay_ms: countdown_secs as u64 * 1000,
            speed: 1.0,
            repeat,
        },
        target_window: None,
    };

    // Simple Repeat is a brand-new macro (no variables), so replace the
    // canonical document outright rather than merging over the current one.
    app_state
        .coordinator
        .set_document(MacroDocument::from(&profile));

    info!(?profile, "Updated document for simple repeat");

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
    hold_delay_ms: Option<u64>,
) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    // Check mutual exclusion: must be idle
    if app_state.coordinator.engine_state() != EngineState::Idle {
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
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
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

    let actual_interval_ms = interval_ms.max(20); // Minimum 20ms interval
    let actual_hold_delay_ms = hold_delay_ms.unwrap_or(150); // Default 150ms hold delay

    let config = KeyClickConfig {
        interval_ms: actual_interval_ms,
        hold_delay_ms: actual_hold_delay_ms,
    };

    let handle = start_key_click_runner(config, input_hook, injector, get_position);
    app_state.key_click_handle = Some(handle);

    info!(
        interval_ms = actual_interval_ms,
        hold_delay_ms = actual_hold_delay_ms,
        "Key-click mode started"
    );

    Ok(())
}

#[tauri::command]
fn stop_key_click(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    // Just signal stop, don't take the handle.
    // The poll_events loop will clean up when it sees !is_running().
    if let Some(ref handle) = app_state.key_click_handle {
        handle.stop();
        info!("Key-click mode stop requested");
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
fn cmd_save_profile(
    state: State<'_, Mutex<AppState>>,
    name: Option<String>,
) -> Result<String, String> {
    let mut app_state = state.lock().unwrap();

    if let Some(n) = name {
        app_state.coordinator.session_mut().set_name(n);
    }

    // Persist the lossless canonical document (variables + metadata included).
    let document = app_state.coordinator.document().clone();
    let path = save_document(&document).map_err(|e| e.to_string())?;
    let _ = save_last_used(&document.name);
    let _ = save_recent(&document.name);

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn cmd_load_profile(state: State<'_, Mutex<AppState>>, name: String) -> Result<Profile, String> {
    let document = load_document(&name).map_err(|e| e.to_string())?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document);
    let _ = save_last_used(&name);
    let _ = save_recent(&name);

    Ok(app_state.coordinator.profile_view())
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
fn cmd_get_recent_profiles() -> Vec<String> {
    load_recent()
}

/// Document metadata exposed to the frontend (lossless edit surface).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DocumentMeta {
    description: Option<String>,
    author: Option<String>,
    tags: Vec<String>,
}

#[tauri::command]
fn cmd_get_document_meta(state: State<'_, Mutex<AppState>>) -> DocumentMeta {
    let app_state = state.lock().unwrap();
    let doc = app_state.coordinator.document();
    DocumentMeta {
        description: doc.description.clone(),
        author: doc.author.clone(),
        tags: doc.tags.clone(),
    }
}

#[tauri::command]
fn cmd_set_document_meta(
    state: State<'_, Mutex<AppState>>,
    description: Option<String>,
    author: Option<String>,
    tags: Vec<String>,
) {
    let mut app_state = state.lock().unwrap();
    app_state
        .coordinator
        .set_metadata(description, author, tags);
}

// === Templates ===

#[tauri::command]
fn cmd_list_templates() -> Vec<templates::TemplateInfo> {
    templates::list_templates()
}

#[tauri::command]
fn cmd_apply_template(state: State<'_, Mutex<AppState>>, id: String) -> Result<Profile, String> {
    let yaml = templates::template_yaml(&id).ok_or_else(|| format!("Unknown template: {id}"))?;
    let document = parse_yaml(yaml).map_err(|e| e.to_string())?;
    validate_profile(&document).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document);
    Ok(app_state.coordinator.profile_view())
}

// === Native file import / export ===

#[tauri::command]
fn cmd_export_yaml_to_path(state: State<'_, Mutex<AppState>>, path: String) -> Result<(), String> {
    let yaml = {
        let app_state = state.lock().unwrap();
        document_to_yaml(app_state.coordinator.document()).map_err(|e| e.to_string())?
    };
    std::fs::write(&path, yaml).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_import_yaml_from_path(
    state: State<'_, Mutex<AppState>>,
    path: String,
) -> Result<Profile, String> {
    let yaml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let document = parse_yaml(&yaml).map_err(|e| e.to_string())?;
    validate_profile(&document).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document);
    Ok(app_state.coordinator.profile_view())
}

#[tauri::command]
fn get_current_profile(state: State<'_, Mutex<AppState>>) -> Profile {
    state.lock().unwrap().coordinator.profile_view()
}

// === Recording Commands ===

#[tauri::command]
fn start_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Idle {
        return Err("Recording already in progress".into());
    }

    if app_state.coordinator.engine_state() != EngineState::Idle {
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
        Some(tap_application::RecorderEvent::RecordingCompleted { timeline }) => timeline,
        _ => Timeline { actions: vec![] },
    };

    info!(
        "Recording stopped, {} actions captured",
        timeline.actions.len()
    );

    // Update the canonical document with the recorded timeline.
    app_state
        .coordinator
        .session_mut()
        .set_recorded_timeline(timeline.clone());
    app_state
        .coordinator
        .session_mut()
        .set_name("Recorded".into());

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
    let picker_window =
        WebviewWindowBuilder::new(&app, "picker", WebviewUrl::App("picker.html".into()))
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
        window
            .close()
            .map_err(|e| format!("Failed to close picker window: {}", e))?;
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

    // Convert the browser's CSS pixels (window.screenX/Y) into the canonical
    // injection coordinate space. On Windows that means scaling logical pixels
    // up to physical pixels; on macOS both spaces are points, so the factor is
    // 1.0. Recording, injection and picking therefore share one coordinate
    // system.
    let scale = tap_platform::browser_to_injection_scale();
    let inject_x = (x as f64 * scale).round() as i32;
    let inject_y = (y as f64 * scale).round() as i32;

    info!(
        "Position picked: css ({}, {}), injection ({}, {}), scale {}",
        x, y, inject_x, inject_y, scale
    );

    // Emit the injection-space coordinates to the main window
    app.emit(
        "position-picked",
        PositionPickedEvent {
            x: inject_x,
            y: inject_y,
        },
    )
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
    // Serialize the lossless canonical document directly (keeps variables/metadata).
    document_to_yaml(app_state.coordinator.document()).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_export_yaml_with_metadata(
    state: State<'_, Mutex<AppState>>,
    description: Option<String>,
    author: Option<String>,
) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    let mut document = app_state.coordinator.document().clone();
    if description.is_some() {
        document.description = description;
    }
    if author.is_some() {
        document.author = author;
    }
    document_to_yaml(&document).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_import_yaml(
    state: State<'_, Mutex<AppState>>,
    yaml_content: String,
) -> Result<Profile, String> {
    // Parse into the canonical document (lossless: keeps variables + metadata).
    let document = parse_yaml(&yaml_content).map_err(|e| e.to_string())?;

    // Validate
    validate_profile(&document).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;

    // Update app state and return the resolved Profile view for display.
    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document);

    Ok(app_state.coordinator.profile_view())
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

    validate_profile(&dsl_profile).map_err(|errors| errors.into_iter().map(|e| e.into()).collect())
}

#[tauri::command]
fn cmd_get_macro_variables(state: State<'_, Mutex<AppState>>) -> Vec<VariableDefinitionResponse> {
    let app_state = state.lock().unwrap();

    // Read variable definitions straight off the canonical document.
    app_state
        .coordinator
        .document()
        .variables
        .iter()
        .map(|(name, def)| VariableDefinitionResponse {
            name: name.clone(),
            var_type: match def.var_type {
                tap_core::VariableType::String => "string".to_string(),
                tap_core::VariableType::Number => "number".to_string(),
                tap_core::VariableType::Boolean => "boolean".to_string(),
            },
            default: def.default.clone(),
            description: def.description.clone(),
        })
        .collect()
}

#[tauri::command]
fn cmd_set_runtime_variables(
    state: State<'_, Mutex<AppState>>,
    vars: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    // Convert the JSON payload into typed run-time overrides applied on the next
    // run (these win over the document's variable defaults in the Resolve stage).
    let mut overrides = std::collections::HashMap::new();
    for (key, value) in vars {
        let typed = if let Some(s) = value.as_str() {
            VariableValue::String(s.to_string())
        } else if let Some(n) = value.as_f64() {
            VariableValue::Number(n)
        } else if let Some(b) = value.as_bool() {
            VariableValue::Boolean(b)
        } else {
            continue;
        };
        overrides.insert(key, typed);
    }

    state
        .lock()
        .unwrap()
        .coordinator
        .set_runtime_vars(overrides);

    Ok(())
}

#[tauri::command]
fn cmd_get_runtime_variables(
    state: State<'_, Mutex<AppState>>,
) -> std::collections::HashMap<String, serde_json::Value> {
    let app_state = state.lock().unwrap();
    let mut result = std::collections::HashMap::new();

    for (key, value) in app_state.coordinator.runtime_vars() {
        let json_val = match value {
            VariableValue::String(s) => serde_json::Value::String(s.clone()),
            VariableValue::Number(n) => serde_json::Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
            ),
            VariableValue::Boolean(b) => serde_json::Value::Bool(*b),
        };
        result.insert(key.clone(), json_val);
    }

    result
}

// === Engine Ports (adapter bridges) ===

/// Bridges the `tap-platform` injector to the application-layer [`ActionExecutor`]
/// port, keeping `tap-application` free of any platform dependency.
struct EnigoExecutor {
    injector: EnigoInjector,
}

impl EnigoExecutor {
    fn new(injector: EnigoInjector) -> Self {
        Self { injector }
    }
}

impl ActionExecutor for EnigoExecutor {
    fn execute(&self, action: &Action) -> Result<(), String> {
        InputInjector::inject(&self.injector, action).map_err(|e| e.to_string())
    }
}

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
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "tap_tauri=debug,tap_core=debug,tap_platform=debug,tauri=info".into()
            }),
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

    // Create the player with executor and platform provider, wrapped in the
    // application-layer coordinator (owns the engine state machine + document).
    let executor = EnigoExecutor::new(injector);
    let player_handle = Player::spawn(executor, platform_provider);
    let coordinator = Coordinator::new(player_handle);

    // Create the recorder
    let recorder = Recorder::with_defaults();

    // Start global mouse tracking
    let mouse_tracker = start_mouse_tracker(MouseTrackerConfig::default());

    // Store handles in app state
    let state = AppState {
        coordinator,
        executed_count: 0,
        current_action_index: None,
        recorder,
        input_hook: None,
        mouse_tracker: Some(mouse_tracker),
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
            app_state.coordinator.drain_events()
        };

        // Process player events
        for event in player_events {
            debug!(?event, "received engine event");

            // Update state
            {
                let mut app_state = state.lock().unwrap();
                match &event {
                    EngineEvent::StateChanged { new, .. } => {
                        app_state.coordinator.set_engine_state(*new);
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

                // Push to recorder; emit recording status to the frontend.
                if let Some(tap_application::RecorderEvent::EventCaptured {
                    event_count,
                    duration_ms,
                }) = app_state
                    .recorder
                    .push_event(raw_event.timestamp_ms, core_event)
                {
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
        let (key_click_events, should_cleanup) = {
            let app_state = state.lock().unwrap();
            let events = app_state
                .key_click_handle
                .as_ref()
                .map(|h| h.drain())
                .unwrap_or_default();
            // Check if handle exists but is no longer running
            let cleanup = app_state
                .key_click_handle
                .as_ref()
                .map(|h| !h.is_running())
                .unwrap_or(false);
            (events, cleanup)
        };

        // Emit events to frontend
        for event in &key_click_events {
            match event {
                KeyClickEvent::Started => {
                    debug!("Key-click mode started event");
                }
                KeyClickEvent::Click { count, x, y } => {
                    debug!(count, x, y, "Key-click: click performed");
                }
                KeyClickEvent::Stopped { total_clicks } => {
                    debug!(total_clicks, "Key-click mode stopped event");
                }
            }

            if let Err(e) = app.emit("key-click-event", event) {
                warn!("Failed to emit key-click event: {}", e);
            }
        }

        // Clean up handle if stopped (separate lock acquisition)
        if should_cleanup {
            let mut app_state = state.lock().unwrap();
            if app_state
                .key_click_handle
                .as_ref()
                .map(|h| !h.is_running())
                .unwrap_or(false)
            {
                app_state.key_click_handle = None;
                debug!("Key-click handle cleaned up");
            }
        }
    }
}

/// Convert platform input event to core raw event type.
fn convert_input_event(event: &InputEventType, last_pos: (i32, i32)) -> RawEventType {
    match event {
        InputEventType::MouseMove { x, y } => RawEventType::MouseMove { x: *x, y: *y },
        InputEventType::MouseDown { x, y, button } => {
            let (px, py) = if *x == 0 && *y == 0 {
                last_pos
            } else {
                (*x, *y)
            };
            RawEventType::MouseDown {
                x: px,
                y: py,
                button: convert_button(*button),
            }
        }
        InputEventType::MouseUp { x, y, button } => {
            let (px, py) = if *x == 0 && *y == 0 {
                last_pos
            } else {
                (*x, *y)
            };
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
    let app_state = state.lock().unwrap();

    // Stop player if running
    app_state.coordinator.emergency_stop();

    // Stop key-click mode if running (just signal, don't take)
    if let Some(ref handle) = app_state.key_click_handle {
        handle.stop();
        info!("Key-click mode stop requested by emergency stop shortcut");
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
        .plugin(tauri_plugin_dialog::init())
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
            if let Err(e) = app.global_shortcut().register(emergency_shortcut) {
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
            cmd_get_recent_profiles,
            cmd_get_document_meta,
            cmd_set_document_meta,
            cmd_list_templates,
            cmd_apply_template,
            cmd_export_yaml_to_path,
            cmd_import_yaml_from_path,
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
