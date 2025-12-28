//! MongoDB connection management

use mongodb::{Client, Database};
use std::sync::Arc;

/// MongoDB connection wrapper
pub struct MongoConnection {
    pub client: Client,
    pub database: Database,
}

impl MongoConnection {
    pub async fn new(uri: &str, db_name: &str) -> anyhow::Result<Self> {
        let client = Client::with_uri_str(uri).await?;
        let database = client.database(db_name);

        // Ping to verify connection
        database.run_command(mongodb::bson::doc! { "ping": 1 }).await?;

        tracing::info!("Connected to MongoDB: {}", db_name);

        Ok(Self { client, database })
    }

    pub fn collection<T: Send + Sync>(&self, name: &str) -> mongodb::Collection<T> {
        self.database.collection(name)
    }
}

/// Initialize MongoDB connection
pub async fn init_mongodb(uri: &str, db_name: &str) -> anyhow::Result<Arc<MongoConnection>> {
    let conn = Arc::new(MongoConnection::new(uri, db_name).await?);
    Ok(conn)
}

