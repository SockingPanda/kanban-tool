use kanban_contract::{ApiBoard, CreateBoardRequest, CreateBoardResponse};

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub fn create_board(&self, request: CreateBoardRequest) -> Result<ApiBoard, ClientError> {
        let response: CreateBoardResponse = self.post("/api/v1/boards", &request)?;
        Ok(response.data)
    }
}
