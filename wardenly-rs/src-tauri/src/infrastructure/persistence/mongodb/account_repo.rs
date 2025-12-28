//! MongoDB Account Repository implementation
//!
//! Uses `_id` as primary key (mapped from Account.id) for MongoDB compatibility.
//! Sorting is consistent with SQLite: ranking DESC, id ASC.

use crate::domain::error::DomainError;
use crate::domain::model::Account;
use crate::domain::repository::{AccountRepository, Result};
use mongodb::bson::{doc, Document};
use mongodb::Collection;
use mongodb::options::FindOptions;
use std::sync::Arc;
use tokio::runtime::Handle;

use super::MongoConnection;

/// MongoDB document wrapper for Account with _id field
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AccountDocument {
    #[serde(rename = "_id")]
    id: String,
    role_name: String,
    user_name: String,
    password: String,
    server_id: i32,
    ranking: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    cookies: Option<Vec<crate::domain::model::Cookie>>,
}

impl From<Account> for AccountDocument {
    fn from(account: Account) -> Self {
        Self {
            id: account.id,
            role_name: account.role_name,
            user_name: account.user_name,
            password: account.password,
            server_id: account.server_id,
            ranking: account.ranking,
            cookies: account.cookies,
        }
    }
}

impl From<AccountDocument> for Account {
    fn from(doc: AccountDocument) -> Self {
        Self {
            id: doc.id,
            role_name: doc.role_name,
            user_name: doc.user_name,
            password: doc.password,
            server_id: doc.server_id,
            ranking: doc.ranking,
            cookies: doc.cookies,
        }
    }
}

pub struct MongoAccountRepository {
    collection: Collection<AccountDocument>,
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
                .find_one(doc! { "_id": id })
                .await
                .map(|opt| opt.map(Account::from))
                .map_err(|e| DomainError::Database(e.to_string()))
        })
    }

    fn find_all(&self) -> Result<Vec<Account>> {
        let collection = self.collection.clone();

        Self::runtime().block_on(async move {
            use futures::TryStreamExt;

            let options = FindOptions::builder()
                .sort(doc! { "ranking": 1, "_id": 1 })
                .build();

            let cursor = collection
                .find(doc! {})
                .with_options(options)
                .await
                .map_err(|e| DomainError::Database(e.to_string()))?;

            let docs: Vec<AccountDocument> = cursor
                .try_collect()
                .await
                .map_err(|e| DomainError::Database(e.to_string()))?;

            Ok(docs.into_iter().map(Account::from).collect())
        })
    }

    fn save(&self, account: &Account) -> Result<()> {
        let collection = self.collection.clone();
        let doc = AccountDocument::from(account.clone());

        Self::runtime().block_on(async move {
            let options = mongodb::options::ReplaceOptions::builder()
                .upsert(true)
                .build();

            collection
                .replace_one(doc! { "_id": &doc.id }, &doc)
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
                .delete_one(doc! { "_id": id })
                .await
                .map_err(|e| DomainError::Database(e.to_string()))?;

            Ok(())
        })
    }
}

