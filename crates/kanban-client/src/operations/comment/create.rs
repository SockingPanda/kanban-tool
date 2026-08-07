use kanban_protocol::{ApiComment, CreateCommentRequest, CreateCommentResponse};

use crate::{
    KanbanClient, error::ClientError, shared::prepare_create_comment_request,
    transport::encode_path_segment,
};

impl KanbanClient {
    pub fn create_comment(
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
        let response: CreateCommentResponse = self.post(
            &format!("/api/v1/tasks/{}/comments", encode_path_segment(task_id)),
            &request,
        )?;
        Ok(response.data)
    }

    pub fn create_comment_by_selector(
        &self,
        board: &str,
        selector: &str,
        request: &CreateCommentRequest,
    ) -> Result<ApiComment, ClientError> {
        let task_id = self.resolve_task_id(board, selector)?;
        self.create_comment(&task_id, request)
    }
}
