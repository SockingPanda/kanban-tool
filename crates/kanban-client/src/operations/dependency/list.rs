use kanban_protocol::{ApiDependencies, ListDependenciesResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn list_dependencies(&self, task_id: &str) -> Result<ApiDependencies, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: ListDependenciesResponse = self.get(&format!(
            "/api/v1/tasks/{}/dependencies",
            encode_path_segment(task_id)
        ))?;
        Ok(response.data)
    }

    pub fn list_dependencies_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<ApiDependencies, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.list_dependencies(&task_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[test]
    fn list_dependency_requires_a_global_task_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_dependencies("default#1")
            .expect_err("board-local selectors must be resolved first");
        assert_eq!(error.code(), "invalid_input");
    }
}
