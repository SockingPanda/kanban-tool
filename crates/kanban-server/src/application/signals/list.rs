use super::support::api_signal;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{BoardLabelPath, MetadataEnvelope, SignalFilterMeta, SignalQuery};
use kanban_service::KanbanError;
use kanban_service::SignalListOptions as ApplicationSignalListOptions;
pub(crate) async fn list_signals(
    state: AppState,
    BoardLabelPath { board }: BoardLabelPath,
    query: SignalQuery,
) -> Result<kanban_protocol::ListSignalsResponse, ApiError> {
    validate_limit(query.limit)?;
    let signals = state
        .application()
        .list_signals(&board, signal_options(&query)?)
        .await?
        .into_iter()
        .map(api_signal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MetadataEnvelope {
        data: signals,
        meta: SignalFilterMeta {
            include_all: query.include_all,
            limit: query.limit,
        },
    })
}
pub(crate) async fn review_signals(
    state: AppState,
    BoardLabelPath { board }: BoardLabelPath,
    mut query: SignalQuery,
) -> Result<kanban_protocol::ReviewSignalsResponse, ApiError> {
    validate_limit(query.limit)?;
    if !query.include_all && query.status.is_empty() {
        query.status = vec!["open".to_owned(), "confirmed".to_owned()];
    }
    let signals = state
        .application()
        .list_signals(&board, signal_options(&query)?)
        .await?
        .into_iter()
        .map(api_signal)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MetadataEnvelope {
        data: signals,
        meta: SignalFilterMeta {
            include_all: query.include_all,
            limit: query.limit,
        },
    })
}
fn signal_options(query: &SignalQuery) -> Result<ApplicationSignalListOptions, ApiError> {
    Ok(ApplicationSignalListOptions {
        statuses: query
            .status
            .iter()
            .map(|value| super::support::parse_status(value))
            .collect::<Result<Vec<_>, _>>()?,
        kinds: query.kind.clone(),
        task_ref: query.task_ref.clone(),
        include_all: query.include_all,
        limit: query.limit,
    })
}
fn validate_limit(limit: usize) -> Result<(), ApiError> {
    if !(1..=100).contains(&limit) {
        return Err(ApiError(KanbanError::InvalidInput(
            "signal list limit must be between 1 and 100".to_owned(),
        )));
    }
    Ok(())
}
