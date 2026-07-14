use std::convert::Infallible;

use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    response::sse::{Event, Sse},
};
use futures_util::stream;
use kanban_contract::{
    ListEventsQuery, ListEventsResponse, NextAfterMeta, StreamEventData, StreamEventsQuery,
};

use crate::error::{ApiError, extractor_error, invalid_input};
use crate::state::AppState;

use super::shared::events_snapshot;

pub(crate) async fn list_events(
    State(state): State<AppState>,
    query: Result<Query<ListEventsQuery>, QueryRejection>,
) -> Result<Json<ListEventsResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let (data, next_after) = events_snapshot(
        &state,
        &query.board,
        query.task_id.clone(),
        query.after,
        query.limit,
    )?;
    Ok(Json(ListEventsResponse::new(
        data,
        NextAfterMeta { next_after },
    )))
}

pub(crate) async fn stream_events(
    State(state): State<AppState>,
    query: Result<Query<StreamEventsQuery>, QueryRejection>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let (data, _next_after): (Vec<StreamEventData>, i64) = events_snapshot(
        &state,
        &query.board,
        query.task_id.clone(),
        query.after,
        query.limit,
    )?;
    let mut frames = Vec::with_capacity(data.len());
    for event in data {
        let frame = Event::default()
            .event(event.kind.clone())
            .id(event.id.to_string())
            .data(serde_json::to_string(&event).map_err(|error| invalid_input(error.to_string()))?);
        frames.push(Ok::<_, Infallible>(frame));
    }
    Ok(Sse::new(stream::iter(frames)))
}
