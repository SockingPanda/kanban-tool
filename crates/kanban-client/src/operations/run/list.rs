use kanban_protocol::ApiRun;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_runs(&self, task_id: &str) -> Result<Vec<ApiRun>, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: kanban_protocol::ListRunsResponse = rpc!(
            self,
            list_runs,
            ListRunsRequest,
            kanban_protocol::ListRunsPath {
                task_id: task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn list_runs_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<Vec<ApiRun>, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.list_runs(&task_id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[tokio::test]
    async fn list_runs_requires_a_global_task_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        for selector in ["default#1", " t_ "] {
            let error = client
                .list_runs(selector)
                .await
                .expect_err("task selectors must be resolved to a global id first");
            assert_eq!(error.code(), "invalid_input");
        }
    }
}
