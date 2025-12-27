use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Account entity - represents a game account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub role_name: String,
    pub user_name: String,
    pub password: String,
    pub server_id: i32,
    pub ranking: i32,
    pub cookies: Option<String>,
}

impl Account {
    pub fn new(
        role_name: String,
        user_name: String,
        password: String,
        server_id: i32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role_name,
            user_name,
            password,
            server_id,
            ranking: 0,
            cookies: None,
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> String {
        format!("{} - {}", self.server_id, self.role_name)
    }
}

