use kanban_protocol::{ApiErrorCode, ErrorBody, rpc};
use kanban_service::object_model::{ObjectError, ObjectErrorCode};
use tonic::Status;

pub(super) fn object(error: ObjectError) -> Status {
    let code = match error.code {
        ObjectErrorCode::InvalidArgument => ApiErrorCode::InvalidInput,
        ObjectErrorCode::NotFound => ApiErrorCode::NotFound,
        ObjectErrorCode::Conflict => ApiErrorCode::Conflict,
        ObjectErrorCode::FailedPrecondition => ApiErrorCode::InvalidTransition,
        ObjectErrorCode::Storage => {
            tracing::error!(error = %error, "object/file storage operation failed");
            return rpc::encode_status(ErrorBody {
                code: ApiErrorCode::Internal,
                message: "storage.operation_failed".into(),
            });
        }
        ObjectErrorCode::CommitUnknown => {
            tracing::warn!(error = %error, "commit acknowledgement unavailable; retry the original identity");
            return rpc::encode_status(ErrorBody {
                code: ApiErrorCode::ServerUnavailable,
                message: "commit.outcome_unknown".into(),
            });
        }
    };
    rpc::encode_status(ErrorBody {
        code,
        message: error.message,
    })
}
pub(super) fn io(error: impl std::fmt::Display) -> Status {
    tracing::error!(%error, "attachment I/O operation failed");
    rpc::encode_status(ErrorBody {
        code: ApiErrorCode::Internal,
        message: "file.io_failed".into(),
    })
}
pub(super) fn actor(
    state: &crate::AppState,
    metadata: &tonic::metadata::MetadataMap,
    body: &str,
) -> Result<String, Status> {
    let context = super::super::context::call_context(metadata)?;
    crate::application::support::request_actor(
        (!body.is_empty()).then_some(body),
        &context,
        state.default_actor(),
    )
    .map_err(super::super::context::service_error)
}

// 复用 canonical 错误映射，保证 google.rpc.Status 与外层 gRPC code 一致。
macro_rules! transport_errors {
    ($( $name:ident($transport:ident, $domain:ident); )*) => {
        $(pub(super) fn $name(message: impl Into<String>) -> Status {
            let message = message.into();
            rpc::encode_status(ErrorBody { code: ApiErrorCode::$domain, message })
        })*
    };
}
transport_errors! {
    invalid(InvalidArgument, InvalidInput);
    missing(NotFound, NotFound);
    conflict(Aborted, Conflict);
    precondition(FailedPrecondition, InvalidTransition);
    exhausted(ResourceExhausted, ServerUnavailable);
    unavailable(Unavailable, ServerUnavailable);
    data_loss(DataLoss, Internal);
}
