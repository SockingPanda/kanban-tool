use kanban_protocol::{
    ListTasksByStatusQuery, ListTasksByStatusResponse, ListTasksQuery, ListTasksResponse,
};

use crate::{
    KanbanClient,
    error::ClientError,
    shared::{list_tasks_by_status_path, list_tasks_path},
};

impl KanbanClient {
    pub fn list_tasks(
        &self,
        board: &str,
        query: &ListTasksQuery,
    ) -> Result<ListTasksResponse, ClientError> {
        self.get(&list_tasks_path(board, query))
    }

    pub fn list_tasks_by_status(
        &self,
        board: &str,
        query: &ListTasksByStatusQuery,
    ) -> Result<ListTasksByStatusResponse, ClientError> {
        self.get(&list_tasks_by_status_path(board, query))
    }
}
