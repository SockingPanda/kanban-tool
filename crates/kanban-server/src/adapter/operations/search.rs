use kanban_application::SearchTasks;
use kanban_core::Result;
use kanban_search::{SearchIndexStatus, SearchQuery, SearchResults};

use crate::adapter::{TursoApplicationStore, store_error};

impl SearchTasks for TursoApplicationStore {
    async fn search_tasks(&self, query: SearchQuery) -> Result<SearchResults> {
        self.store.search_tasks(query).await.map_err(store_error)
    }

    async fn search_index_status(&self, board: &str) -> Result<SearchIndexStatus> {
        self.store
            .search_index_status(board)
            .await
            .map_err(store_error)
    }

    async fn rebuild_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        self.store
            .rebuild_search_index(board)
            .await
            .map_err(store_error)
    }

    async fn sync_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        self.store
            .sync_search_index(board)
            .await
            .map_err(store_error)
    }
}
