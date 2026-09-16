use kanban_live_core::{Card, LiveError, Resume, Status};
use kanban_rpc_proto::v1 as pb;
use prost::Message;
use tonic::{Code, Status as RpcStatus};

pub const MAX_FRAME_BYTES: usize = 512 * 1024;
pub fn to_status(value: Status) -> i32 {
    match value {
        Status::Triage => 1, Status::Todo => 2, Status::Scheduled => 3, Status::Ready => 4,
        Status::Running => 5, Status::Blocked => 6, Status::Review => 7, Status::Done => 8, Status::Archived => 9,
    }
}
pub fn to_card(card: &Card) -> pb::TaskCard {
    pb::TaskCard { id: card.id.clone(), title: card.title.clone(), status: to_status(card.status),
        priority: card.priority, position: card.position, seq: card.seq, lock_version: card.lock_version }
}
pub fn to_cursor(cursor: &Resume) -> pb::Cursor {
    pb::Cursor { epoch: cursor.epoch.clone(), scope: cursor.scope.clone(), revision: cursor.revision }
}
pub fn envelope(board: &str, cursor: &Resume, body: pb::board_frame::Body) -> Result<pb::BoardFrame, RpcStatus> {
    let frame = pb::BoardFrame { board_id: board.to_owned(), epoch: cursor.epoch.clone(), scope: cursor.scope.clone(), body: Some(body) };
    if frame.encoded_len() > MAX_FRAME_BYTES { return Err(error(LiveError::Budget("流消息过大".into()))); }
    Ok(frame)
}
pub fn error(value: LiveError) -> RpcStatus {
    let (code, stable, retryable) = match &value {
        LiveError::Invalid(_) => (Code::InvalidArgument, "invalid_input", false),
        LiveError::NotFound(_) => (Code::NotFound, "not_found", false),
        LiveError::Conflict(_) => (Code::Aborted, "version_conflict", false),
        LiveError::Budget(_) => (Code::ResourceExhausted, "budget_exceeded", false),
        LiveError::Unavailable => (Code::Unavailable, "source_unavailable", true),
        LiveError::Stopped => (Code::Unavailable, "host_stopped", true),
    };
    let mut status = RpcStatus::new(code, value.to_string());
    status.metadata_mut().insert_bin("kb-error-bin", tonic::metadata::MetadataValue::from_bytes(
        &pb::ErrorDetail { code: stable.into(), retryable }.encode_to_vec()));
    status
}
