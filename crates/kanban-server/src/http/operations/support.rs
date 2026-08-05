use crate::error::ApiError;
use axum::http::HeaderMap;
use kanban_core::KanbanError;

pub(super) fn request_actor(
    body_actor: Option<&str>,
    headers: &HeaderMap,
    default_actor: &str,
) -> Result<String, ApiError> {
    let actor = match body_actor {
        Some(actor) => actor,
        None => headers
            .get("x-kb-actor")
            .map(|value| {
                value.to_str().map_err(|_| {
                    KanbanError::InvalidInput("x-kb-actor must contain valid text".to_owned())
                })
            })
            .transpose()?
            .unwrap_or(default_actor),
    };
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(KanbanError::InvalidInput("actor is required".to_owned()).into());
    }
    Ok(actor.to_owned())
}
