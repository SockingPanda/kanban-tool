use super::support::*;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ListDependenciesPath, ListDependenciesResponse};
pub(crate) async fn list_dependencies(
    state: AppState,
    ListDependenciesPath { task_id }: ListDependenciesPath,
) -> Result<ListDependenciesResponse, ApiError> {
    let dependencies = state.application().list_dependencies(&task_id).await?;
    Ok(ListDependenciesResponse {
        data: api_dependencies(dependencies)?,
    })
}
