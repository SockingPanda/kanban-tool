use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};
use kanban_search::{SearchIndexStatus, SearchQuery, SearchResults};

use crate::{ApplicationService, ApplicationStore};

/// Search 与 projection maintenance 的共享 application port。
pub trait SearchTasks: ApplicationStore {
    fn search_tasks(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<SearchResults>> + Send;

    fn search_index_status(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<SearchIndexStatus>> + Send;

    fn rebuild_search_index(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<SearchIndexStatus>> + Send;

    fn sync_search_index(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<SearchIndexStatus>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: SearchTasks,
    C: Clock,
{
    pub async fn search_tasks(&self, query: SearchQuery) -> Result<SearchResults> {
        let query = query.normalized();
        query
            .validate()
            .map_err(|error| KanbanError::InvalidInput(error.to_string()))?;
        self.store.search_tasks(query).await
    }

    pub async fn search_index_status(&self, board: &str) -> Result<SearchIndexStatus> {
        let board = normalize_board(board)?;
        self.store.search_index_status(&board).await
    }

    pub async fn rebuild_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        let board = normalize_board(board)?;
        self.store.rebuild_search_index(&board).await
    }

    pub async fn sync_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        let board = normalize_board(board)?;
        self.store.sync_search_index(&board).await
    }
}

fn normalize_board(board: &str) -> Result<String> {
    let board = board.trim();
    if board.is_empty() {
        return Err(KanbanError::InvalidInput("board is required".to_owned()));
    }
    Ok(board.to_owned())
}
