use kanban_protocol::{ApiBoard, ArchiveBoardRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn archive_board(
        &self,
        board: &str,
        request: &ArchiveBoardRequest,
    ) -> Result<ApiBoard, ClientError> {
        let response: kanban_protocol::ArchiveBoardResponse = rpc!(
            self,
            archive_board,
            ArchiveBoardRequest,
            kanban_protocol::ArchiveBoardPath {
                board: board.trim().to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
}
