use kanban_protocol::ApiDependencies;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_dependencies(&self, task_id: &str) -> Result<ApiDependencies, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: kanban_protocol::ListDependenciesResponse = rpc!(
            self,
            list_dependencies,
            ListDependenciesRequest,
            kanban_protocol::ListDependenciesPath {
                task_id: task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn list_dependencies_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiDependencies, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.list_dependencies(&task_id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[tokio::test]
    async fn list_dependency_requires_a_global_task_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_dependencies("default#1")
            .await
            .expect_err("board-local selectors must be resolved first");
        assert_eq!(error.code(), "invalid_input");
    }
}
