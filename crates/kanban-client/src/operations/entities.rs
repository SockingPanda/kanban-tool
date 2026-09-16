use kanban_protocol::CliEntity;

use crate::{KanbanClient, error::ClientError, transport::rpc};

pub use kanban_protocol::EntityUpsertRequest;

impl KanbanClient {
    pub async fn list_entities(
        &self,
        board: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<CliEntity>, ClientError> {
        let response: kanban_protocol::EntityListResponse = rpc!(
            self,
            list_entities,
            ListEntitiesRequest,
            (),
            kanban_protocol::EntityListQuery {
                board: board.map(str::to_owned),
                kind: kind.map(str::to_owned),
                limit
            },
            ()
        )?;
        Ok(response.data)
    }

    pub async fn get_entity(&self, uri: &str) -> Result<CliEntity, ClientError> {
        let response: kanban_protocol::EntityResponse = rpc!(
            self,
            get_entity,
            GetEntityRequest,
            kanban_protocol::EntityPath {
                uri: uri.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn upsert_entity(
        &self,
        request: EntityUpsertRequest,
    ) -> Result<CliEntity, ClientError> {
        let response: kanban_protocol::EntityResponse = rpc!(
            self,
            upsert_entity,
            UpsertEntityRequest,
            (),
            (),
            request.clone()
        )?;
        Ok(response.data)
    }
}
