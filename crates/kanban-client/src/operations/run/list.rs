use kanban_protocol::{ApiRun, ListRunsResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn list_runs(&self, task_id: &str) -> Result<Vec<ApiRun>, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: ListRunsResponse = self.get(&format!(
            "/api/v1/tasks/{}/runs",
            encode_path_segment(task_id)
        ))?;
        Ok(response.data)
    }

    pub fn list_runs_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<Vec<ApiRun>, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.list_runs(&task_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[test]
    fn list_runs_requires_a_global_task_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        for selector in ["default#1", " t_ "] {
            let error = client
                .list_runs(selector)
                .expect_err("task selectors must be resolved to a global id first");
            assert_eq!(error.code(), "invalid_input");
        }
    }
}
