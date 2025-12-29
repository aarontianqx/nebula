//! MongoDB Group Repository implementation
//!
//! Uses `_id` as primary key (mapped from Group.id) for MongoDB compatibility.
//! Sorting is consistent with SQLite: ranking DESC, name ASC.

use crate::domain::error::DomainError;
use crate::domain::model::Group;
use crate::domain::repository::{GroupRepository, Result};
use mongodb::bson::doc;
use mongodb::Collection;
use mongodb::options::FindOptions;
use std::sync::Arc;
use tokio::runtime::Handle;

use super::MongoConnection;

/// MongoDB document wrapper for Group with _id field
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct GroupDocument {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    account_ids: Vec<String>,
    ranking: i32,
}

impl From<Group> for GroupDocument {
    fn from(group: Group) -> Self {
        Self {
            id: group.id,
            name: group.name,
            description: group.description,
            account_ids: group.account_ids,
            ranking: group.ranking,
        }
    }
}

impl From<GroupDocument> for Group {
    fn from(doc: GroupDocument) -> Self {
        Self {
            id: doc.id,
            name: doc.name,
            description: doc.description,
            account_ids: doc.account_ids,
            ranking: doc.ranking,
        }
    }
}

pub struct MongoGroupRepository {
    collection: Collection<GroupDocument>,
    /// Tokio runtime handle captured at creation time.
    runtime: Handle,
}

impl MongoGroupRepository {
    pub fn new(conn: Arc<MongoConnection>, runtime: Handle) -> Self {
        Self {
            collection: conn.collection("groups"),
            runtime,
        }
    }
}

impl GroupRepository for MongoGroupRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Group>> {
        let collection = self.collection.clone();
        let id = id.to_string();
        let runtime = self.runtime.clone();

        // Use block_in_place to safely call block_on from within tokio runtime.
        // This prevents deadlock when called from Tauri command handlers.
        tokio::task::block_in_place(|| {
            runtime.block_on(async move {
                collection
                    .find_one(doc! { "_id": id })
                    .await
                    .map(|opt| opt.map(Group::from))
                    .map_err(|e| DomainError::Database(e.to_string()))
            })
        })
    }

    fn find_all(&self) -> Result<Vec<Group>> {
        let collection = self.collection.clone();
        let runtime = self.runtime.clone();

        tokio::task::block_in_place(|| {
            runtime.block_on(async move {
                use futures::TryStreamExt;

                let options = FindOptions::builder()
                    .sort(doc! { "ranking": 1, "name": 1 })
                    .build();

                let cursor = collection
                    .find(doc! {})
                    .with_options(options)
                    .await
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                let docs: Vec<GroupDocument> = cursor
                    .try_collect()
                    .await
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                Ok(docs.into_iter().map(Group::from).collect())
            })
        })
    }

    fn save(&self, group: &Group) -> Result<()> {
        let collection = self.collection.clone();
        let doc = GroupDocument::from(group.clone());
        let runtime = self.runtime.clone();

        tokio::task::block_in_place(|| {
            runtime.block_on(async move {
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
        })
    }

    fn delete(&self, id: &str) -> Result<()> {
        let collection = self.collection.clone();
        let id = id.to_string();
        let runtime = self.runtime.clone();

        tokio::task::block_in_place(|| {
            runtime.block_on(async move {
                collection
                    .delete_one(doc! { "_id": id })
                    .await
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                Ok(())
            })
        })
    }
}


