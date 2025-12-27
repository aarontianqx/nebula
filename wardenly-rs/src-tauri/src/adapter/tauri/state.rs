use crate::application::coordinator::Coordinator;
use crate::application::eventbus::SharedEventBus;
use crate::application::input::{ClickEvent, InputEventProcessor};
use crate::application::service::{AccountService, GroupService};
use crate::infrastructure::persistence::sqlite::{
    DbConnection, SqliteAccountRepository, SqliteGroupRepository,
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct AppState {
    pub account_service: AccountService<SqliteAccountRepository>,
    pub group_service: GroupService<SqliteGroupRepository>,
    pub coordinator: Arc<Coordinator<SqliteAccountRepository>>,
    pub event_bus: SharedEventBus,
    pub input_processor: Arc<InputEventProcessor>,
    pub click_rx: Arc<Mutex<mpsc::UnboundedReceiver<ClickEvent>>>,
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

        let (input_processor, click_rx) = InputEventProcessor::new();

        Self {
            account_service: AccountService::new(account_repo),
            group_service: GroupService::new(group_repo),
            coordinator,
            event_bus,
            input_processor: Arc::new(input_processor),
            click_rx: Arc::new(Mutex::new(click_rx)),
        }
    }
}
