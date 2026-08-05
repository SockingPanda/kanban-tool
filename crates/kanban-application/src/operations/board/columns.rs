use std::future::Future;

use kanban_core::{Clock, Result};

use crate::{ApplicationService, ApplicationStore, BoardColumnRecord};

pub trait BoardColumns: ApplicationStore {
    fn list_board_columns(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<Vec<BoardColumnRecord>>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: BoardColumns,
    C: Clock,
{
    pub async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
        self.store.list_board_columns(board).await
    }
}

#[cfg(test)]
mod tests {
    use kanban_core::Result;

    use crate::operations::test_support::StubStore;
    use crate::*;

    impl BoardColumns for StubStore {
        async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
            assert_eq!(board, "default");
            Ok(vec![BoardColumnRecord {
                id: "col_default_todo".into(),
                board_id: "b_default".into(),
                status: kanban_core::TaskStatus::Todo,
                title: "Todo".into(),
                position: 20,
                hidden: false,
                wip_limit: None,
                created_at: 1,
                updated_at: 1,
            }])
        }
    }
}
