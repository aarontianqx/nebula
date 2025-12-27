use crate::adapter::tauri::error::ApiError;
use crate::adapter::tauri::state::AppState;
use crate::domain::model::{Account, Group};
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

