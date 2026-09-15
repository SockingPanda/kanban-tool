use kanban_protocol::{BuildContextQuery, BuildContextResponse};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    /// 通过 canonical host 构建只读上下文包。
    pub async fn build_context(
        &self,
        task_id: &str,
        query: &BuildContextQuery,
    ) -> Result<BuildContextResponse, ClientError> {
        if task_id.trim().is_empty() {
            return Err(ClientError::InvalidInput("task_id 不能为空".to_owned()));
        }
        let response: kanban_protocol::BuildContextResponse = rpc!(
            self,
            build_context,
            BuildContextRequest,
            kanban_protocol::BuildContextPath {
                task_id: task_id.to_owned()
            },
            query.clone(),
            ()
        )?;
        Ok(response)
    }
}
