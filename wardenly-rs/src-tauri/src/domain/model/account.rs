use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Account entity - represents a game account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub role_name: String,
    pub user_name: String,
    pub password: String,
    pub server_id: i32,
    pub ranking: i32,
}

impl Account {
    pub fn new(
        role_name: String,
        user_name: String,
        password: String,
        server_id: i32,
        ranking: i32,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            role_name,
            user_name,
            password,
            server_id,
            ranking,
        }
    }

    /// Returns identity string in format "ServerID - RoleName"
    pub fn identity(&self) -> String {
        format!("{} - {}", self.server_id, self.role_name)
    }
}

