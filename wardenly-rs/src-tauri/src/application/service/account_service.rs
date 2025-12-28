use crate::domain::model::Account;
use crate::domain::repository::{AccountRepository, Result};

pub struct AccountService<R: AccountRepository> {
    repo: R,
}

impl<R: AccountRepository> AccountService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn get_all(&self) -> Result<Vec<Account>> {
        self.repo.find_all()
    }

    pub fn create(&self, account: Account) -> Result<Account> {
        self.repo.save(&account)?;
        Ok(account)
    }

    pub fn update(&self, account: Account) -> Result<Account> {
        self.repo.save(&account)?;
        Ok(account)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.repo.delete(id)
    }
}

