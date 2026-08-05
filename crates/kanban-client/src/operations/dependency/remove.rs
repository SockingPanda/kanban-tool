use kanban_protocol::{ApiDependencies, RemoveDependencyResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn remove_dependency(
        &self,
        child_task_id: &str,
        parent_task_id: &str,
    ) -> Result<ApiDependencies, ClientError> {
        let child_task_id = child_task_id.trim();
        let parent_task_id = parent_task_id.trim();
        if !child_task_id.starts_with("t_") || child_task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "task selector must resolve to a global t_... id".to_owned(),
            ));
        }
        if !parent_task_id.starts_with("t_") || parent_task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "task selector must resolve to a global t_... id".to_owned(),
            ));
        }
        let response: RemoveDependencyResponse = self.delete(&format!(
            "/api/v1/tasks/{}/dependencies/{}",
            encode_path_segment(child_task_id),
            encode_path_segment(parent_task_id)
        ))?;
        Ok(response.data)
    }

    pub fn remove_dependency_by_selector(
        &self,
        board: &str,
        child_selector: &str,
        parent_selector: &str,
    ) -> Result<ApiDependencies, ClientError> {
        let child_task_id = self.resolve_task_id(board, child_selector)?;
        let parent_task_id = self.resolve_task_id(board, parent_selector)?;
        self.remove_dependency(&child_task_id, &parent_task_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[test]
    fn remove_dependency_requires_global_parent_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .remove_dependency("t_child", "#1")
            .expect_err("parent selector must be resolved first");
        assert_eq!(error.code(), "invalid_input");
    }
}
