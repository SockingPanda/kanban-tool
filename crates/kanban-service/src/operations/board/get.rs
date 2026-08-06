use kanban_core::{Clock, KanbanError, Result};

use crate::{BoardRecord, KanbanService};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn get_board(&self, selector: &str) -> Result<BoardRecord> {
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(KanbanError::InvalidInput("看板不能为空".to_owned()));
        }
        self.store
            .get_board(selector)
            .await
            .map(super::application_board)
            .map_err(crate::error::store_error)
    }
}
