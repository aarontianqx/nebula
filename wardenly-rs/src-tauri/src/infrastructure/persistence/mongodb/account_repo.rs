//! MongoDB Account Repository implementation

use crate::domain::error::DomainError;
use crate::domain::model::Account;
use crate::domain::repository::{AccountRepository, Result};
use mongodb::bson::doc;
use mongodb::Collection;
use std::sync::Arc;
use tokio::runtime::Handle;

use super::MongoConnection;

pub struct MongoAccountRepository {
    collection: Collection<Account>,
}

impl MongoAccountRepository {
    pub fn new(conn: Arc<MongoConnection>) -> Self {
        Self {
            collection: conn.collection("accounts"),
        }
    }

    fn runtime() -> Handle {
        Handle::current()
    }
}

impl AccountRepository for MongoAccountRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>> {
        let collection = self.collection.clone();
        let id = id.to_string();

        Self::runtime().block_on(async move {
            collection
                .find_one(doc! { "id": id })
                .await
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_all(&self) -> Result<Vec<Account>> {
        let collection = self.collection.clone();

        Self::runtime().block_on(async move {
            use futures::TryStreamExt;

            let cursor = collection
                .find(doc! {})
                .await
                .map_err(|e| DomainError::Database(e.to_string()))?;

            cursor
                .try_collect()
                .await
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn save(&self, account: &Account) -> Result<()> {
        let collection = self.collection.clone();
        let account = account.clone();

        Self::runtime().block_on(async move {
            let options = mongodb::options::ReplaceOptions::builder()
                .upsert(true)
                .build();

            collection
                .replace_one(doc! { "id": &account.id }, &account)
                .with_options(options)
                .await
                .map_err(|e| DomainError::Database(e.to_string()))?;

            Ok(())
        })
    }

    fn delete(&self, id: &str) -> Result<()> {
        let collection = self.collection.clone();
        let id = id.to_string();

        Self::runtime().block_on(async move {
            collection
                .delete_one(doc! { "id": id })
                .await
                .map_err(|e| DomainError::Database(e.to_string()))?;

            Ok(())
        })
    }
}

