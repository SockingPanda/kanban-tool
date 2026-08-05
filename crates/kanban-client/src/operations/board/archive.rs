use kanban_contract::{ApiBoard, ArchiveBoardRequest, ArchiveBoardResponse};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    pub fn archive_board(
        &self,
        board: &str,
        request: &ArchiveBoardRequest,
    ) -> Result<ApiBoard, ClientError> {
        let path = format!(
            "/api/v1/boards/{}/archive",
            encode_path_segment(board.trim())
        );
        let response: ArchiveBoardResponse = self.post(&path, request)?;
        Ok(response.data)
    }
}
