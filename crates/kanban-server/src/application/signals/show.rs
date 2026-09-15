use super::support::api_signal;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{DataEnvelope, GetSignalResponse, SignalPath};
pub(crate) async fn get_signal(
    state: AppState,
    SignalPath { signal_id }: SignalPath,
) -> Result<GetSignalResponse, ApiError> {
    let signal = state.application().get_signal(&signal_id).await?;
    Ok(DataEnvelope {
        data: api_signal(signal)?,
    })
}
