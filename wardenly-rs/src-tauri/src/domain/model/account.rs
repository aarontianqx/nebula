use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Browser cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
}

/// Account entity - represents a game account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub role_name: String,
    pub user_name: String,
    pub password: String,
    pub server_id: i32,
    pub ranking: i32,
    pub cookies: Option<Vec<Cookie>>,
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

    /// Returns identity string in format "ServerID - RoleName"
    pub fn identity(&self) -> String {
        format!("{} - {}", self.server_id, self.role_name)
    }
}

