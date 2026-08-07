use kanban_protocol::{
    SearchStatusResponse, SearchTasksByStatusResponse, SearchTasksQuery, SearchTasksResponse,
};

use crate::{KanbanClient, error::ClientError, shared::search_tasks_path};

impl KanbanClient {
    pub fn search_tasks(
        &self,
        query: &SearchTasksQuery,
    ) -> Result<SearchTasksResponse, ClientError> {
        self.get(&search_tasks_path(query, false))
    }

    pub fn search_tasks_by_status(
        &self,
        query: &SearchTasksQuery,
    ) -> Result<SearchTasksByStatusResponse, ClientError> {
        self.get(&search_tasks_path(query, true))
    }

    pub fn search_status(&self, board: &str) -> Result<SearchStatusResponse, ClientError> {
        self.get(&format!(
            "/api/v1/search/status?board={}",
            crate::transport::encode_path_segment(board.trim())
        ))
    }

    pub fn rebuild_search_index(&self, board: &str) -> Result<SearchStatusResponse, ClientError> {
        self.post(
            &format!(
                "/api/v1/search/index/rebuild?board={}",
                crate::transport::encode_path_segment(board.trim())
            ),
            &serde_json::json!({}),
        )
    }

    pub fn sync_search_index(&self, board: &str) -> Result<SearchStatusResponse, ClientError> {
        self.post(
            &format!(
                "/api/v1/search/index/sync?board={}",
                crate::transport::encode_path_segment(board.trim())
            ),
            &serde_json::json!({}),
        )
    }
}
