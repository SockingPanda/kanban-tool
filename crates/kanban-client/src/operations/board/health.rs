use kanban_protocol::HealthReport;

use crate::transport::rpc;

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub async fn health(&self) -> Result<HealthReport, ClientError> {
        let response: kanban_protocol::HealthResponse =
            rpc!(self, get_health, GetHealthRequest, (), (), ())?;
        Ok(response.data)
    }
}
