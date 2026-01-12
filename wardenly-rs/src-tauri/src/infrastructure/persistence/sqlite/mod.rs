mod account_repo;
mod group_repo;

pub use account_repo::SqliteAccountRepository;
pub use group_repo::SqliteGroupRepository;

use crate::infrastructure::config::paths::default_sqlite_path;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type DbConnection = Arc<Mutex<Connection>>;

/// Initialize the SQLite database
pub fn init_database() -> anyhow::Result<DbConnection> {
    let db_path = default_sqlite_path();

    // Ensure directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    tracing::info!("Initializing database at {:?}", db_path);

    let conn = Connection::open(&db_path)?;

    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            role_name TEXT NOT NULL,
            user_name TEXT NOT NULL,
            password TEXT NOT NULL,
            server_id INTEGER NOT NULL,
            ranking INTEGER DEFAULT 0
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            account_ids TEXT NOT NULL,
            ranking INTEGER DEFAULT 0
        )",
        [],
    )?;

    tracing::info!("Database initialized successfully");

    Ok(Arc::new(Mutex::new(conn)))
}

