use kanban_contract::{HealthReport, HealthResponse};

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub fn health(&self) -> Result<HealthReport, ClientError> {
        let response: HealthResponse = self.get("/health")?;
        Ok(response.data)
    }
}
