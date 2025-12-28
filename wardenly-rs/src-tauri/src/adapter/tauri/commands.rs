use crate::adapter::tauri::error::ApiError;
use crate::adapter::tauri::state::AppState;
use crate::domain::model::{Account, Group, ScriptInfo, SessionInfo};
use crate::infrastructure::config::resources;
use serde::Deserialize;
use tauri::State;

// ====== Account Commands ======

#[tauri::command]
pub fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    state
        .account_service
        .get_all()
        .map_err(|e| ApiError::from(e).into())
}

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    role_name: String,
    user_name: String,
    password: String,
    server_id: i32,
    #[serde(default)]
    ranking: i32,
}

#[tauri::command]
pub fn create_account(
    state: State<'_, AppState>,
    request: CreateAccountRequest,
) -> Result<Account, String> {
    let account = Account::new(
        request.role_name,
        request.user_name,
        request.password,
        request.server_id,
        request.ranking,
    );

    state
        .account_service
        .create(account)
        .map_err(|e| ApiError::from(e).into())
}

#[tauri::command]
pub fn update_account(
    state: State<'_, AppState>,
    account: Account,
) -> Result<Account, String> {
    state
        .account_service
        .update(account)
        .map_err(|e| ApiError::from(e).into())
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .account_service
        .delete(&id)
        .map_err(|e| ApiError::from(e).into())
}

// ====== Group Commands ======

#[tauri::command]
pub fn get_groups(state: State<'_, AppState>) -> Result<Vec<Group>, String> {
    state
        .group_service
        .get_all()
        .map_err(|e| ApiError::from(e).into())
}

#[derive(Deserialize)]
pub struct CreateGroupRequest {
    name: String,
    description: Option<String>,
    #[serde(default)]
    ranking: i32,
}

#[tauri::command]
pub fn create_group(
    state: State<'_, AppState>,
    request: CreateGroupRequest,
) -> Result<Group, String> {
    let mut group = Group::new(request.name, request.ranking);
    group.description = request.description;

    state
        .group_service
        .create(group)
        .map_err(|e| ApiError::from(e).into())
}

#[tauri::command]
pub fn update_group(state: State<'_, AppState>, group: Group) -> Result<Group, String> {
    state
        .group_service
        .update(group)
        .map_err(|e| ApiError::from(e).into())
}

#[tauri::command]
pub fn delete_group(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .group_service
        .delete(&id)
        .map_err(|e| ApiError::from(e).into())
}

/// Run all accounts in a group sequentially with 3 second intervals.
/// Skips accounts that already have a running session.
#[tauri::command]
pub async fn run_group(state: State<'_, AppState>, group_id: String) -> Result<(), String> {
    // Get group from service
    let groups = state.group_service.get_all().map_err(|e| e.to_string())?;
    let group = groups
        .iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| format!("Group not found: {}", group_id))?
        .clone();

    if group.account_ids.is_empty() {
        return Err("Group has no accounts".to_string());
    }

    // Get all accounts to sort by ranking
    let accounts = state.account_service.get_all().map_err(|e| e.to_string())?;

    // Filter and sort accounts by ranking
    let mut group_accounts: Vec<_> = accounts
        .into_iter()
        .filter(|a| group.account_ids.contains(&a.id))
        .collect();
    group_accounts.sort_by(|a, b| a.ranking.cmp(&b.ranking).then_with(|| a.id.cmp(&b.id)));

    // Clone coordinator for async block
    let coordinator = state.coordinator.clone();

    // Spawn background task to start accounts sequentially
    tauri::async_runtime::spawn(async move {
        for (i, account) in group_accounts.iter().enumerate() {
            // Check if session already exists for this account
            let sessions = coordinator.get_sessions().await;
            let already_running = sessions.iter().any(|s| s.account_id == account.id);

            if already_running {
                tracing::info!(
                    "Skipping account {} - already has a running session",
                    account.id
                );
                continue;
            }

            // Create and start session
            match coordinator.create_session(&account.id).await {
                Ok(session_id) => {
                    if let Err(e) = coordinator.start_session(&session_id).await {
                        tracing::error!("Failed to start session {}: {}", session_id, e);
                    } else {
                        tracing::info!(
                            "Started session for account {} ({}/{})",
                            account.id,
                            i + 1,
                            group_accounts.len()
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create session for account {}: {}", account.id, e);
                }
            }

            // Wait 3 seconds before starting next account (except for last one)
            if i < group_accounts.len() - 1 {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }
        tracing::info!("Finished starting group accounts");
    });

    Ok(())
}

// ====== Session Commands ======

#[tauri::command]
pub async fn get_sessions(state: State<'_, AppState>) -> Result<Vec<SessionInfo>, String> {
    Ok(state.coordinator.get_sessions().await)
}

#[tauri::command]
pub async fn start_session(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<String, String> {
    // Create and start session
    let session_id = state
        .coordinator
        .create_session(&account_id)
        .await
        .map_err(|e| e.to_string())?;

    state
        .coordinator
        .start_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(session_id)
}

#[tauri::command]
pub async fn stop_session(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .coordinator
        .stop_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_all_sessions(state: State<'_, AppState>) -> Result<(), String> {
    state.coordinator.stop_all().await;
    Ok(())
}

#[tauri::command]
pub async fn click_session(
    state: State<'_, AppState>,
    session_id: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    state
        .coordinator
        .click_session(&session_id, x, y)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn drag_session(
    state: State<'_, AppState>,
    session_id: String,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
) -> Result<(), String> {
    state
        .coordinator
        .drag_session(&session_id, (from_x, from_y), (to_x, to_y))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn click_all_sessions(state: State<'_, AppState>, x: f64, y: f64) -> Result<(), String> {
    state.coordinator.click_all(x, y).await;
    Ok(())
}

#[tauri::command]
pub async fn refresh_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state
        .coordinator
        .refresh_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_screencast(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state
        .coordinator
        .start_screencast(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_screencast(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state
        .coordinator
        .stop_screencast(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn capture_screenshot(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state
        .coordinator
        .capture_screenshot(&session_id)
        .await
        .map_err(|e| e.to_string())
}

// ====== Script Commands ======

#[tauri::command]
pub fn get_scripts() -> Result<Vec<ScriptInfo>, String> {
    let scripts = resources::load_scripts().map_err(|e| e.to_string())?;
    Ok(scripts.iter().map(ScriptInfo::from).collect())
}

#[tauri::command]
pub async fn start_script(
    state: State<'_, AppState>,
    session_id: String,
    script_name: String,
) -> Result<(), String> {
    state
        .coordinator
        .start_script(&session_id, &script_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_script(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    state
        .coordinator
        .stop_script(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_all_scripts(
    state: State<'_, AppState>,
    script_name: String,
) -> Result<(), String> {
    state.coordinator.start_all_scripts(&script_name).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_all_scripts(state: State<'_, AppState>) -> Result<(), String> {
    state.coordinator.stop_all_scripts().await;
    Ok(())
}

// ====== Input Commands ======

#[tauri::command]
pub async fn set_keyboard_passthrough(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    state
        .input_processor
        .set_enabled(enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_keyboard_passthrough_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.input_processor.is_enabled().await)
}

#[tauri::command]
pub async fn update_cursor_position(
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    in_bounds: bool,
) -> Result<(), String> {
    state
        .input_processor
        .update_cursor(x, y, in_bounds)
        .await;
    Ok(())
}

#[tauri::command]
pub async fn set_active_session_for_input(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<(), String> {
    state
        .input_processor
        .set_active_session(session_id)
        .await;
    Ok(())
}

// ====== Theme Commands ======

use crate::infrastructure::config::{ThemeConfig, ThemeResponse};

#[tauri::command]
pub fn get_theme_config() -> Result<ThemeResponse, String> {
    use crate::infrastructure::config::loader::load_config;
    
    let config: ThemeConfig = load_config("themes");
    let active_theme_name = config.active_theme.clone();
    
    // Get the active theme, or fall back to default
    let theme = config
        .themes
        .get(&active_theme_name)
        .cloned()
        .unwrap_or_default();
    
    let available_themes: Vec<String> = config.themes.keys().cloned().collect();
    
    Ok(ThemeResponse {
        active_theme: active_theme_name,
        css_vars: theme.to_css_vars(),
        available_themes,
    })
}

