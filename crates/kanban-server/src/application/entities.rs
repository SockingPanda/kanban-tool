use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    CliEntity, CliEntityListOutput, CliEntityShowOutput, DataEnvelope, EntityListQuery, EntityPath,
    EntityUpsertRequest,
};
use kanban_service::dto::EntityRecord;
use kanban_service::operations::{EntityListOptions, EntityUpsertCommand};
pub(crate) async fn list_entities(
    state: AppState,
    query: EntityListQuery,
) -> Result<CliEntityListOutput, ApiError> {
    let entities = state
        .application()
        .list_entities(EntityListOptions {
            board: query.board,
            kind: query.kind,
            limit: query.limit,
        })
        .await?
        .into_iter()
        .map(api_entity)
        .collect();
    Ok(DataEnvelope::new(entities))
}
pub(crate) async fn get_entity(
    state: AppState,
    EntityPath { uri }: EntityPath,
) -> Result<CliEntityShowOutput, ApiError> {
    let entity = state.application().get_entity(&uri).await?;
    Ok(DataEnvelope::new(api_entity(entity)))
}
pub(crate) async fn upsert_entity(
    state: AppState,
    request: EntityUpsertRequest,
) -> Result<CliEntityShowOutput, ApiError> {
    let entity = state
        .application()
        .upsert_entity(EntityUpsertCommand {
            uri: request.uri,
            kind: request.kind,
            source_table: request.source_table,
            source_id: request.source_id,
            board: request.board,
            task_id: request.task_id,
            title: request.title,
            summary: request.summary,
            content_hash: request.content_hash,
            archived_at: request.archived_at,
        })
        .await?;
    Ok(DataEnvelope::new(api_entity(entity)))
}
fn api_entity(entity: EntityRecord) -> CliEntity {
    CliEntity {
        uri: entity.uri,
        kind: entity.kind,
        source_table: entity.source_table,
        source_id: entity.source_id,
        board_id: entity.board_id,
        task_id: entity.task_id,
        title: entity.title,
        summary: entity.summary,
        content_hash: entity.content_hash,
        created_at: entity.created_at,
        updated_at: entity.updated_at,
        archived_at: entity.archived_at,
    }
}
