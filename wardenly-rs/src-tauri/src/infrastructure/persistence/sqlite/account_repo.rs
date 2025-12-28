use crate::domain::error::DomainError;
use crate::domain::model::{Account, Cookie};
use crate::domain::repository::{AccountRepository, Result};
use super::DbConnection;
use rusqlite::params;

pub struct SqliteAccountRepository {
    pub conn: DbConnection,
}

impl SqliteAccountRepository {
    pub fn new(conn: DbConnection) -> Self {
        Self { conn }
    }
}

/// Parse cookies from JSON string stored in database
fn parse_cookies(json_str: Option<String>) -> Option<Vec<Cookie>> {
    json_str.and_then(|s| {
        if s.is_empty() {
            None
        } else {
            serde_json::from_str(&s).ok()
        }
    })
}

/// Serialize cookies to JSON string for database storage
fn serialize_cookies(cookies: &Option<Vec<Cookie>>) -> Option<String> {
    cookies.as_ref().and_then(|c| {
        if c.is_empty() {
            None
        } else {
            serde_json::to_string(c).ok()
        }
    })
}

impl AccountRepository for SqliteAccountRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, role_name, user_name, password, server_id, ranking, cookies 
             FROM accounts WHERE id = ?"
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            let cookies_json: Option<String> = row.get(6)?;
            Ok(Some(Account {
                id: row.get(0)?,
                role_name: row.get(1)?,
                user_name: row.get(2)?,
                password: row.get(3)?,
                server_id: row.get(4)?,
                ranking: row.get(5)?,
                cookies: parse_cookies(cookies_json),
            }))
        } else {
            Ok(None)
        }
    }

    fn find_all(&self) -> Result<Vec<Account>> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, role_name, user_name, password, server_id, ranking, cookies 
             FROM accounts ORDER BY ranking ASC, id ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let cookies_json: Option<String> = row.get(6)?;
            Ok(Account {
                id: row.get(0)?,
                role_name: row.get(1)?,
                user_name: row.get(2)?,
                password: row.get(3)?,
                server_id: row.get(4)?,
                ranking: row.get(5)?,
                cookies: parse_cookies(cookies_json),
            })
        })?;

        let mut accounts = Vec::new();
        for account in rows {
            accounts.push(account?);
        }

        Ok(accounts)
    }

    fn save(&self, account: &Account) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let cookies_json = serialize_cookies(&account.cookies);

        conn.execute(
            "INSERT OR REPLACE INTO accounts 
             (id, role_name, user_name, password, server_id, ranking, cookies)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                account.id,
                account.role_name,
                account.user_name,
                account.password,
                account.server_id,
                account.ranking,
                cookies_json,
            ],
        )?;

        Ok(())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        conn.execute("DELETE FROM accounts WHERE id = ?", params![id])?;

        Ok(())
    }
}

