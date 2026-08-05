use kanban_contract::{ApiBoardColumn, ListBoardColumnsResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn list_board_columns(&self, board: &str) -> Result<Vec<ApiBoardColumn>, ClientError> {
        let path = format!("/api/v1/boards/{}/columns", encode_path_segment(board));
        let response: ListBoardColumnsResponse = self.get(&path)?;
        Ok(response.data)
    }
}
