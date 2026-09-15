use kanban_protocol::ApiComment;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_comments(&self, task_id: &str) -> Result<Vec<ApiComment>, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let response: kanban_protocol::ListCommentsResponse = rpc!(
            self,
            list_comments,
            ListCommentsRequest,
            kanban_protocol::ListCommentsPath {
                task_id: task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn list_comments_by_selector(
        &self,
        board: &str,
        selector: &str,
    ) -> Result<Vec<ApiComment>, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.list_comments(&task_id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[tokio::test]
    async fn list_comments_requires_a_global_task_id_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        let error = client
            .list_comments("default#1")
            .await
            .expect_err("board-local selectors must be resolved first");
        assert_eq!(error.code(), "invalid_input");
    }
}
