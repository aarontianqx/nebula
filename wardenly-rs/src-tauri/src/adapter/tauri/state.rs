use crate::application::coordinator::Coordinator;
use crate::application::eventbus::SharedEventBus;
use crate::application::service::{AccountService, GroupService};
use crate::infrastructure::persistence::sqlite::{
    DbConnection, SqliteAccountRepository, SqliteGroupRepository,
};
use std::sync::Arc;

pub struct AppState {
    pub account_service: AccountService<SqliteAccountRepository>,
    pub group_service: GroupService<SqliteGroupRepository>,
    pub coordinator: Arc<Coordinator<SqliteAccountRepository>>,
    pub event_bus: SharedEventBus,
}

impl AppState {
    pub fn new(conn: DbConnection, event_bus: SharedEventBus) -> Self {
        let account_repo = SqliteAccountRepository::new(conn.clone());
        let group_repo = SqliteGroupRepository::new(conn);

        let account_repo_for_coordinator = SqliteAccountRepository::new(account_repo.conn.clone());
        let coordinator = Arc::new(Coordinator::new(
            event_bus.clone(),
            Arc::new(account_repo_for_coordinator),
        ));

        Self {
            account_service: AccountService::new(account_repo),
            group_service: GroupService::new(group_repo),
            coordinator,
            event_bus,
        }
    }
}
