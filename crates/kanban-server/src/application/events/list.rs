use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    ListEventsQuery, ListEventsResponse, NextAfterMeta, StreamEventData,
    event_payload::EventPayload,
};
use kanban_service::KanbanError;
use kanban_service::{EventListOptions as ApplicationEventListOptions, EventRecord};
use serde_json::Value;
pub(crate) async fn list_events(
    state: AppState,
    query: ListEventsQuery,
) -> Result<ListEventsResponse, ApiError> {
    let page = state
        .application()
        .list_events(
            &query.board,
            ApplicationEventListOptions {
                task_id: query.task_id,
                after: query.after,
                limit: query.limit,
            },
        )
        .await?;
    let data = page
        .events
        .into_iter()
        .map(api_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ListEventsResponse::new(
        data,
        NextAfterMeta {
            next_after: page.next_after,
        },
    ))
}
pub(crate) fn api_event(event: EventRecord) -> Result<StreamEventData, ApiError> {
    let payload_value: Value = serde_json::from_str(&event.payload_json).map_err(|error| {
        KanbanError::Storage(format!(
            "存储的 event payload 对 {} 不是有效 JSON：{error}",
            event.event_id
        ))
    })?;
    let payload =
        EventPayload::from_kind_and_value(&event.kind, payload_value).map_err(|error| {
            KanbanError::Storage(format!(
                "存储的 event payload 对 {} 无效：{error}",
                event.kind
            ))
        })?;
    Ok(StreamEventData {
        id: event.id,
        event_id: event.event_id,
        board_id: event.board_id,
        task_id: event.task_id,
        run_id: event.run_id,
        kind: event.kind,
        actor: event.actor,
        payload,
        created_at: event.created_at,
    })
}
