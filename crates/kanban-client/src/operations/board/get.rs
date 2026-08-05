use kanban_protocol::{ApiBoard, GetBoardResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn get_board(&self, board: &str) -> Result<ApiBoard, ClientError> {
        let path = format!("/api/v1/boards/{}", encode_path_segment(board.trim()));
        let response: GetBoardResponse = self.get(&path)?;
        Ok(response.data)
    }
}
