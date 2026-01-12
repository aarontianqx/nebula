use crate::domain::error::DomainError;
use crate::domain::model::Account;
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

impl AccountRepository for SqliteAccountRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, role_name, user_name, password, server_id, ranking 
             FROM accounts WHERE id = ?"
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Account {
                id: row.get(0)?,
                role_name: row.get(1)?,
                user_name: row.get(2)?,
                password: row.get(3)?,
                server_id: row.get(4)?,
                ranking: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_all(&self) -> Result<Vec<Account>> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, role_name, user_name, password, server_id, ranking 
             FROM accounts ORDER BY ranking ASC, id ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                role_name: row.get(1)?,
                user_name: row.get(2)?,
                password: row.get(3)?,
                server_id: row.get(4)?,
                ranking: row.get(5)?,
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

        conn.execute(
            "INSERT OR REPLACE INTO accounts 
             (id, role_name, user_name, password, server_id, ranking)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                account.id,
                account.role_name,
                account.user_name,
                account.password,
                account.server_id,
                account.ranking,
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

