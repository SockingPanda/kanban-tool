use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, BoardRecord};

pub trait BoardGet: ApplicationStore {
    fn get_board(&self, selector: &str) -> impl Future<Output = Result<BoardRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: BoardGet,
    C: Clock,
{
    pub async fn get_board(&self, selector: &str) -> Result<BoardRecord> {
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(KanbanError::InvalidInput("看板不能为空".to_owned()));
        }
        self.store.get_board(selector).await
    }
}
