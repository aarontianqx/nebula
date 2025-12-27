//! MongoDB persistence implementation
//!
//! This module is only compiled when the `mongodb` feature is enabled.

#[cfg(feature = "mongodb")]
mod account_repo;
#[cfg(feature = "mongodb")]
mod group_repo;
#[cfg(feature = "mongodb")]
mod connection;

#[cfg(feature = "mongodb")]
pub use account_repo::MongoAccountRepository;
#[cfg(feature = "mongodb")]
pub use group_repo::MongoGroupRepository;
#[cfg(feature = "mongodb")]
pub use connection::{MongoConnection, init_mongodb};

