use crate::application::coordinator::Coordinator;
use crate::application::eventbus::SharedEventBus;
use crate::application::input::{ClickEvent, InputEventProcessor};
use crate::application::service::{AccountService, GroupService};
use crate::domain::repository::{AccountRepository, GroupRepository};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Type aliases for dynamic repository types
pub type DynAccountRepository = Box<dyn AccountRepository>;
pub type DynGroupRepository = Box<dyn GroupRepository>;

pub struct AppState {
    pub account_service: AccountService<DynAccountRepository>,
    pub group_service: GroupService<DynGroupRepository>,
    pub coordinator: Arc<Coordinator>,
    pub event_bus: SharedEventBus,
    pub input_processor: Arc<InputEventProcessor>,
    pub click_rx: Arc<Mutex<mpsc::UnboundedReceiver<ClickEvent>>>,
}

impl AppState {
    pub fn new(
        account_repo: DynAccountRepository,
        group_repo: DynGroupRepository,
        coordinator_account_repo: Arc<dyn AccountRepository>,
        event_bus: SharedEventBus,
    ) -> Self {
        let coordinator = Arc::new(Coordinator::new(
            event_bus.clone(),
            coordinator_account_repo,
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
