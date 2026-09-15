use kanban_protocol::{ApiComment, CreateCommentRequest};

use crate::{
    KanbanClient, error::ClientError, shared::prepare_create_comment_request, transport::rpc,
};

impl KanbanClient {
    pub async fn create_comment(
        &self,
        task_id: &str,
        request: &CreateCommentRequest,
    ) -> Result<ApiComment, ClientError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(ClientError::InvalidInput(
                "任务选择器必须解析为全局 t_... ID".to_owned(),
            ));
        }
        let request = prepare_create_comment_request(request.clone(), task_id);
        let response: kanban_protocol::CreateCommentResponse = rpc!(
            self,
            create_comment,
            CreateCommentRequest,
            kanban_protocol::CreateCommentPath {
                task_id: task_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn create_comment_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &CreateCommentRequest,
    ) -> Result<ApiComment, ClientError> {
        let task_id = self.resolve_task_id(board, selector).await?;
        self.create_comment(&task_id, request).await
    }
}
