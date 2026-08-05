use kanban_contract::{ListTasksQuery, ListTasksResponse};

use crate::{KanbanClient, error::ClientError, shared::list_tasks_path};

impl KanbanClient {
    pub fn list_tasks(
        &self,
        board: &str,
        query: &ListTasksQuery,
    ) -> Result<ListTasksResponse, ClientError> {
        self.get(&list_tasks_path(board, query))
    }
}
