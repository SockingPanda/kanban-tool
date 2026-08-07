use kanban_protocol::{ApiBoard, ListBoardsResponse};

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub fn list_boards(&self, include_archived: bool) -> Result<Vec<ApiBoard>, ClientError> {
        let path = format!("/api/v1/boards?include_archived={include_archived}");
        let response: ListBoardsResponse = self.get(&path)?;
        Ok(response.data)
    }
}
