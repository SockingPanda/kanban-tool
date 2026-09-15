use kanban_protocol::ApiBoardColumn;

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn list_board_columns(
        &self,
        board: &str,
    ) -> Result<Vec<ApiBoardColumn>, ClientError> {
        let response: kanban_protocol::ListBoardColumnsResponse = rpc!(
            self,
            list_board_columns,
            ListBoardColumnsRequest,
            kanban_protocol::ListBoardColumnsPath {
                board: board.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }
}
