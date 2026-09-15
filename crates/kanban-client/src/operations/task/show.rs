use kanban_protocol::{ApiTask, TaskDetailAggregate};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn get_task(&self, task_id: &str) -> Result<ApiTask, ClientError> {
        let response: kanban_protocol::GetTaskResponse = rpc!(
            self,
            get_task,
            GetTaskRequest,
            kanban_protocol::GetTaskPath {
                task_id: task_id.trim().to_owned()
            },
            kanban_protocol::GetTaskQuery::default(),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn get_task_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiTask, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.get_task(&task_id).await
    }

    pub async fn get_task_details(
        &self,
        task_id: &str,
    ) -> Result<TaskDetailAggregate, ClientError> {
        let response: kanban_protocol::GetTaskDetailsResponse = rpc!(
            self,
            get_task_details,
            GetTaskDetailsRequest,
            kanban_protocol::GetTaskPath {
                task_id: task_id.trim().to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn get_task_details_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<TaskDetailAggregate, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.get_task_details(&task_id).await
    }
}
