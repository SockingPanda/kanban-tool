use kanban_protocol::{ApiBoard, CreateBoardRequest};

use crate::transport::rpc;

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub async fn create_board(&self, request: CreateBoardRequest) -> Result<ApiBoard, ClientError> {
        let response: kanban_protocol::CreateBoardResponse = rpc!(
            self,
            create_board,
            CreateBoardRequest,
            (),
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
}
