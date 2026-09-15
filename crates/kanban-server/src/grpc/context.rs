//! RPC metadata 与稳定业务错误的传输适配。
use crate::{application::support::CallContext, error::ApiError};
use kanban_protocol::{ApiErrorCode, ErrorBody, rpc};
use tonic::{Status, metadata::MetadataMap};

pub(super) fn call_context(metadata: &MetadataMap) -> Result<CallContext, Status> {
    let mut plain = metadata.get_all("x-kb-actor").iter();
    let actor = plain
        .next()
        .map(|value| value.to_str())
        .transpose()
        .map_err(invalid_request)?;
    if plain.next().is_some() {
        return Err(invalid_request("重复的 x-kb-actor metadata"));
    }
    let mut binary = metadata.get_all_bin("x-kb-actor-bin").iter();
    let bytes = binary
        .next()
        .map(|value| value.to_bytes())
        .transpose()
        .map_err(invalid_request)?;
    if binary.next().is_some() {
        return Err(invalid_request("重复的 x-kb-actor-bin metadata"));
    }
    let unicode = bytes
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(invalid_request)?;
    if let (Some(plain), Some(unicode)) = (actor, unicode)
        && plain != unicode
    {
        return Err(invalid_request("actor metadata 不一致"));
    }
    let actor = unicode.or(actor);
    if actor.is_some_and(|actor| actor.len() > 8192 || actor.trim().is_empty()) {
        return Err(invalid_request("actor 必须为非空文本且不超过 8192 字节"));
    }
    Ok(CallContext {
        actor: actor.map(str::to_owned),
    })
}

pub(super) fn invalid_request(error: impl std::fmt::Display) -> Status {
    rpc::encode_status(ErrorBody {
        code: ApiErrorCode::InvalidInput,
        message: error.to_string(),
    })
}

pub(super) fn service_error(error: ApiError) -> Status {
    rpc::encode_status(error.into_wire())
}

pub(super) fn response_codec_error(error: impl std::fmt::Display) -> Status {
    tracing::error!(%error, "application response 无法转换为 RPC 契约");
    rpc::encode_status(ErrorBody {
        code: ApiErrorCode::Internal,
        message: "服务端内部错误，请稍后重试。".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataValue;

    #[test]
    fn actor_metadata_preserves_unicode_and_rejects_ambiguity() {
        let mut metadata = MetadataMap::new();
        metadata.insert_bin(
            "x-kb-actor-bin",
            MetadataValue::from_bytes("张三".as_bytes()),
        );
        assert_eq!(
            call_context(&metadata).unwrap().actor.as_deref(),
            Some("张三")
        );
        metadata.insert("x-kb-actor", "other".parse().unwrap());
        let error = call_context(&metadata).unwrap_err();
        assert_eq!(
            rpc::decode_status(&error).unwrap().code,
            ApiErrorCode::InvalidInput
        );
        metadata.remove_bin("x-kb-actor-bin");
        metadata.append("x-kb-actor", "other".parse().unwrap());
        assert!(call_context(&metadata).is_err());
    }

    #[test]
    fn invalid_actor_bytes_and_storage_details_do_not_escape() {
        let mut metadata = MetadataMap::new();
        metadata.insert_bin("x-kb-actor-bin", MetadataValue::from_bytes(&[0xff]));
        assert!(call_context(&metadata).is_err());
        let error = service_error(
            kanban_service::KanbanError::Storage("private/path.db".to_owned()).into(),
        );
        let decoded = rpc::decode_status(&error).unwrap();
        assert_eq!(decoded.code, ApiErrorCode::Internal);
        assert!(!decoded.message.contains("private"));
    }
}
