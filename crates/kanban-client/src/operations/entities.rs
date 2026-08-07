use kanban_protocol::{CliEntity, CliEntityListOutput, CliEntityShowOutput};
use serde::Serialize;

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityUpsertRequest {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    pub board: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub archived_at: Option<i64>,
}

impl KanbanClient {
    pub fn list_entities(
        &self,
        board: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<CliEntity>, ClientError> {
        let mut query = vec![format!("limit={limit}")];
        if let Some(board) = board {
            query.push(format!("board={}", encode_path_segment(board)));
        }
        if let Some(kind) = kind {
            query.push(format!("kind={}", encode_path_segment(kind)));
        }
        let response: CliEntityListOutput =
            self.get(&format!("/api/v1/entities?{}", query.join("&")))?;
        Ok(response.data)
    }

    pub fn get_entity(&self, uri: &str) -> Result<CliEntity, ClientError> {
        let response: CliEntityShowOutput =
            self.get(&format!("/api/v1/entities/{}", encode_path_segment(uri)))?;
        Ok(response.data)
    }

    pub fn upsert_entity(&self, request: EntityUpsertRequest) -> Result<CliEntity, ClientError> {
        let response: CliEntityShowOutput = self.put("/api/v1/entities", &request)?;
        Ok(response.data)
    }
}
