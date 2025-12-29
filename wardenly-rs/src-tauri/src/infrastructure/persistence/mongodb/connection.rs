//! MongoDB connection management
//!
//! Provides connection pooling and timeout configuration for MongoDB operations.
//! The `MongoConnection` wrapper holds a reference to the database and provides
//! collection access methods.

use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use std::sync::Arc;
use std::time::Duration;

/// Connection timeout for MongoDB operations.
/// Applies to both initial connection and server selection.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);

/// MongoDB connection wrapper.
/// Holds the database reference for collection operations.
pub struct MongoConnection {
    database: Database,
}

impl MongoConnection {
    /// Create a new MongoDB connection with timeout configuration.
    ///
    /// # Arguments
    /// * `uri` - MongoDB connection URI (e.g., "mongodb://localhost:27017")
    /// * `db_name` - Database name to use
    ///
    /// # Errors
    /// Returns an error if:
    /// - The URI is invalid
    /// - Connection cannot be established within timeout
    /// - Database ping fails
    pub async fn new(uri: &str, db_name: &str) -> anyhow::Result<Self> {
        let options = Self::create_client_options(uri).await?;
        let client = Client::with_options(options)?;
        let database = client.database(db_name);

        // Ping to verify connection is actually working
        database
            .run_command(mongodb::bson::doc! { "ping": 1 })
            .await?;

        tracing::info!("Connected to MongoDB: {}", db_name);

        Ok(Self { database })
    }

    /// Get a typed collection from the database.
    pub fn collection<T: Send + Sync>(&self, name: &str) -> mongodb::Collection<T> {
        self.database.collection(name)
    }

    /// Create client options with timeout configuration.
    async fn create_client_options(uri: &str) -> anyhow::Result<ClientOptions> {
        let mut options = ClientOptions::parse(uri).await?;
        options.connect_timeout = Some(CONNECTION_TIMEOUT);
        options.server_selection_timeout = Some(CONNECTION_TIMEOUT);
        Ok(options)
    }
}

/// Initialize MongoDB connection and return a shared reference.
pub async fn init_mongodb(uri: &str, db_name: &str) -> anyhow::Result<Arc<MongoConnection>> {
    let conn = Arc::new(MongoConnection::new(uri, db_name).await?);
    Ok(conn)
}

/// Test MongoDB connection without initializing repositories.
///
/// Used by the Settings UI to validate connection before saving.
/// Returns Ok(()) if connection is successful, Err with detailed message otherwise.
pub async fn test_connection(uri: &str, db_name: &str) -> Result<(), String> {
    MongoConnection::new(uri, db_name)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
