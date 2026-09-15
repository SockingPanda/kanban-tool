use kanban_protocol::ApiBoard;

use crate::transport::rpc;

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub async fn list_boards(&self, include_archived: bool) -> Result<Vec<ApiBoard>, ClientError> {
        let response: kanban_protocol::ListBoardsResponse = rpc!(
            self,
            list_boards,
            ListBoardsRequest,
            (),
            kanban_protocol::ListBoardsQuery { include_archived },
            ()
        )?;
        Ok(response.data)
    }
}
