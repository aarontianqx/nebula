use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Group entity - represents a collection of accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub account_ids: Vec<String>,
    pub ranking: i32,
}

impl Group {
    pub fn new(name: String, ranking: i32) -> Self {
        Self {
            id: Ulid::new().to_string(),
            name,
            description: None,
            account_ids: Vec::new(),
            ranking,
        }
    }
}

