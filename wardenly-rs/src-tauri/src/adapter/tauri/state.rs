use crate::application::service::{AccountService, GroupService};
use crate::infrastructure::persistence::sqlite::{
    DbConnection, SqliteAccountRepository, SqliteGroupRepository,
};

pub struct AppState {
    pub account_service: AccountService<SqliteAccountRepository>,
    pub group_service: GroupService<SqliteGroupRepository>,
}

impl AppState {
    pub fn new(conn: DbConnection) -> Self {
        let account_repo = SqliteAccountRepository::new(conn.clone());
        let group_repo = SqliteGroupRepository::new(conn);

        Self {
            account_service: AccountService::new(account_repo),
            group_service: GroupService::new(group_repo),
        }
    }
}

