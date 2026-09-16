use kanban_protocol::ApiBoard;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn get_board(&self, board: &str) -> Result<ApiBoard, ClientError> {
        let response: kanban_protocol::GetBoardResponse = rpc!(
            self,
            get_board,
            GetBoardRequest,
            kanban_protocol::GetBoardPath {
                board: board.trim().to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }
}
