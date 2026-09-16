use kanban_protocol::ApiDependencies;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
    ) -> Result<ApiDependencies, ClientError> {
        let child_task_id = child_task_id.trim();
        let parent_task_id = parent_task_id.trim();
        if !child_task_id.starts_with("t_") || child_task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        if !parent_task_id.starts_with("t_") || parent_task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: kanban_protocol::RemoveDependencyResponse = rpc!(
            self,
            remove_dependency,
            RemoveDependencyRequest,
            kanban_protocol::RemoveDependencyPath {
                child_task_id: child_task_id.to_owned(),
                parent_task_id: parent_task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn remove_dependency_by_selector(
        &self,
        board: &str,
        child_selector: &str,
        parent_selector: &str,
    ) -> Result<ApiDependencies, ClientError> {
        let child_task_id = self.resolve_task_id(board, child_selector).await?;
        let parent_task_id = self.resolve_task_id(board, parent_selector).await?;
        self.remove_dependency(&child_task_id, &parent_task_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[tokio::test]
    async fn remove_dependency_requires_global_parent_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .remove_dependency("t_child", "#1")
            .await
            .expect_err("parent selector must be resolved first");
        assert_eq!(error.code(), "invalid_input");
    }
}
