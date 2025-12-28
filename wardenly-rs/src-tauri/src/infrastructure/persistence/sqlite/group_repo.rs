use crate::domain::error::DomainError;
use crate::domain::model::Group;
use crate::domain::repository::{GroupRepository, Result};
use super::DbConnection;
use rusqlite::params;

pub struct SqliteGroupRepository {
    conn: DbConnection,
}

impl SqliteGroupRepository {
    pub fn new(conn: DbConnection) -> Self {
        Self { conn }
    }
}

impl GroupRepository for SqliteGroupRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Group>> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, name, description, account_ids, ranking 
             FROM groups WHERE id = ?"
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            let account_ids_json: String = row.get(3)?;
            let account_ids: Vec<String> = serde_json::from_str(&account_ids_json)
                .unwrap_or_default();

            Ok(Some(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                account_ids,
                ranking: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_all(&self) -> Result<Vec<Group>> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, name, description, account_ids, ranking 
             FROM groups ORDER BY ranking ASC, name ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let account_ids_json: String = row.get(3)?;
            let account_ids: Vec<String> = serde_json::from_str(&account_ids_json)
                .unwrap_or_default();

            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                account_ids,
                ranking: row.get(4)?,
            })
        })?;

        let mut groups = Vec::new();
        for group in rows {
            groups.push(group?);
        }

        Ok(groups)
    }

    fn save(&self, group: &Group) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        let account_ids_json = serde_json::to_string(&group.account_ids)
            .map_err(|e| DomainError::Database(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO groups 
             (id, name, description, account_ids, ranking)
             VALUES (?, ?, ?, ?, ?)",
            params![
                group.id,
                group.name,
                group.description,
                account_ids_json,
                group.ranking,
            ],
        )?;

        Ok(())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| DomainError::Database(e.to_string()))?;

        conn.execute("DELETE FROM groups WHERE id = ?", params![id])?;

        Ok(())
    }
}

