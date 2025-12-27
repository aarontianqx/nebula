use crate::adapter::tauri::error::ApiError;
use crate::adapter::tauri::state::AppState;
use crate::domain::model::{Account, Group, SessionInfo};
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
}

#[tauri::command]
pub fn create_group(
    state: State<'_, AppState>,
    request: CreateGroupRequest,
) -> Result<Group, String> {
    let mut group = Group::new(request.name);
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

