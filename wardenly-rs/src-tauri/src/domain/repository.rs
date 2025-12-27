use crate::domain::error::DomainError;
use crate::domain::model::{Account, Group};

pub type Result<T> = std::result::Result<T, DomainError>;

/// Repository trait for Account entity
#[allow(dead_code)]
pub trait AccountRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>>;
    fn find_all(&self) -> Result<Vec<Account>>;
    fn save(&self, account: &Account) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}

/// Repository trait for Group entity
#[allow(dead_code)]
pub trait GroupRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Result<Option<Group>>;
    fn find_all(&self) -> Result<Vec<Group>>;
    fn save(&self, group: &Group) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}

