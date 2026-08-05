use kanban_protocol::{BuildContextPath, BuildContextQuery, BuildContextResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    /// Build a read-only context pack through the canonical host.
    pub fn build_context(
        &self,
        task_id: &str,
        query: &BuildContextQuery,
    ) -> Result<BuildContextResponse, ClientError> {
        if task_id.trim().is_empty() {
            return Err(ClientError::InvalidInput("task_id 不能为空".to_owned()));
        }
        let path = BuildContextPath {
            task_id: task_id.to_owned(),
        };
        let mut uri = format!(
            "/api/v1/tasks/{}/context?board={}&lexical_limit={}&graph_limit={}&vector_limit={}&max_items={}&depth={}",
            encode_path_segment(&path.task_id),
            encode_path_segment(&query.board),
            query.lexical_limit,
            query.graph_limit,
            query.vector_limit,
            query.max_items,
            query.depth,
        );
        if let Some(value) = query.task.as_deref() {
            uri.push_str("&task=");
            uri.push_str(&encode_path_segment(value));
        }
        if let Some(value) = query.reference.as_deref() {
            uri.push_str("&reference=");
            uri.push_str(&encode_path_segment(value));
        }
        if let Some(value) = query.query.as_deref() {
            uri.push_str("&query=");
            uri.push_str(&encode_path_segment(value));
        }
        if let Some(value) = query.budget {
            uri.push_str("&budget=");
            uri.push_str(&value.to_string());
        }
        self.get(&uri)
    }
}
