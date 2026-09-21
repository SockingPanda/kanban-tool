use kanban_protocol::{
    ListTasksByStatusQuery, ListTasksByStatusResponse, ListTasksQuery, ListTasksResponse,
};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_tasks(
        &self,
        board: &str,
        query: &ListTasksQuery,
    ) -> Result<ListTasksResponse, ClientError> {
        if !query.module_ids.is_empty() || query.cycle_id.is_some() {
            self.require_planning(true).await?;
        }
        let response: kanban_protocol::ListTasksResponse = rpc!(
            self,
            list_tasks,
            ListTasksRequest,
            kanban_protocol::ListTasksPath {
                board: board.to_owned()
            },
            query.clone(),
            ()
        )?;
        Ok(response)
    }

    pub async fn list_tasks_by_status(
        &self,
        board: &str,
        query: &ListTasksByStatusQuery,
    ) -> Result<ListTasksByStatusResponse, ClientError> {
        if !query.module_ids.is_empty() || query.cycle_id.is_some() {
            self.require_planning(true).await?;
        }
        let response: kanban_protocol::ListTasksByStatusResponse = rpc!(
            self,
            list_tasks_by_status,
            ListTasksByStatusRequest,
            kanban_protocol::ListTasksByStatusPath {
                board: board.to_owned()
            },
            query.clone(),
            ()
        )?;
        Ok(response)
    }
}
