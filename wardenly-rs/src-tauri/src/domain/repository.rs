use crate::domain::error::DomainError;
use crate::domain::model::{Account, Group};

pub type Result<T> = std::result::Result<T, DomainError>;

/// Repository trait for Account entity
pub trait AccountRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>>;
    fn find_all(&self) -> Result<Vec<Account>>;
    fn save(&self, account: &Account) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}

/// Repository trait for Group entity
pub trait GroupRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> Result<Option<Group>>;
    fn find_all(&self) -> Result<Vec<Group>>;
    fn save(&self, group: &Group) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
}

// Implement AccountRepository for Box<dyn AccountRepository> to allow dynamic dispatch
impl AccountRepository for Box<dyn AccountRepository> {
    fn find_by_id(&self, id: &str) -> Result<Option<Account>> {
        (**self).find_by_id(id)
    }

    fn find_all(&self) -> Result<Vec<Account>> {
        (**self).find_all()
    }

    fn save(&self, account: &Account) -> Result<()> {
        (**self).save(account)
    }

    fn delete(&self, id: &str) -> Result<()> {
        (**self).delete(id)
    }
}

// Implement GroupRepository for Box<dyn GroupRepository> to allow dynamic dispatch
impl GroupRepository for Box<dyn GroupRepository> {
    fn find_by_id(&self, id: &str) -> Result<Option<Group>> {
        (**self).find_by_id(id)
    }

    fn find_all(&self) -> Result<Vec<Group>> {
        (**self).find_all()
    }

    fn save(&self, group: &Group) -> Result<()> {
        (**self).save(group)
    }

    fn delete(&self, id: &str) -> Result<()> {
        (**self).delete(id)
    }
}

