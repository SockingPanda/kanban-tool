use kanban_live_core::{
    Resume,
    query::{QueryBytes, QueryNext},
};
use kanban_protocol::rpc::v1::{self as pb, query_frame::Body};
use std::sync::Arc;

use kanban_protocol::rpc::query::QUERY_CHUNK_BYTES as CHUNK_BYTES;

/// 每次 poll 只生成一帧；中途取消只丢弃当前传输，客户端的已提交 cursor 不变。
pub(super) struct Transfer {
    begin: pb::QueryBegin,
    bytes: Arc<QueryBytes>,
    index: usize,
    started: bool,
}

impl Transfer {
    pub fn new(value: QueryNext) -> Option<Self> {
        let (cursor, base, snapshot, result_size, hash, offset, delete, bytes) = match value {
            QueryNext::Snapshot(value) => (
                value.cursor,
                None,
                true,
                value.bytes.as_ref().as_ref().len(),
                value.sha256,
                0,
                0,
                value.bytes,
            ),
            QueryNext::Delta(value) => (
                value.cursor.clone(),
                Some(value.base.clone()),
                false,
                value.result_size,
                value.result_sha256,
                value.offset,
                value.delete_length,
                value.patch.clone(),
            ),
            QueryNext::Idle => return None,
        };
        let size = bytes.as_ref().as_ref().len();
        Some(Self {
            begin: pb::QueryBegin {
                cursor: Some(to_cursor(cursor)),
                base: base.map(to_cursor),
                snapshot,
                result_size: result_size as u64,
                result_sha256: hash.to_vec(),
                offset: offset as u64,
                delete_length: delete as u64,
                patch_size: size as u64,
                chunk_count: size.div_ceil(CHUNK_BYTES) as u32,
            },
            bytes,
            index: 0,
            started: false,
        })
    }

    pub fn next(&mut self) -> Body {
        if !self.started {
            self.started = true;
            return Body::Begin(self.begin.clone());
        }
        let bytes = self.bytes.as_ref().as_ref();
        let offset = self.index * CHUNK_BYTES;
        if offset < bytes.len() {
            let data = bytes[offset..bytes.len().min(offset + CHUNK_BYTES)].to_vec();
            let index = self.index as u32;
            self.index += 1;
            Body::Chunk(pb::QueryChunk { index, data })
        } else {
            Body::End(pb::QueryEnd {
                cursor: self.begin.cursor.clone(),
            })
        }
    }
}

pub(super) fn to_cursor(cursor: Resume) -> pb::QueryCursor {
    pb::QueryCursor {
        epoch: cursor.epoch,
        scope: cursor.scope,
        revision: cursor.revision,
    }
}

pub(super) fn from_cursor(cursor: pb::QueryCursor) -> Resume {
    Resume {
        epoch: cursor.epoch,
        scope: cursor.scope,
        revision: cursor.revision,
    }
}

pub(super) fn failure(status: tonic::Status) -> Body {
    use kanban_protocol::{ApiErrorCode, rpc};
    let retryable = matches!(
        status.code(),
        tonic::Code::Unavailable
            | tonic::Code::DeadlineExceeded
            | tonic::Code::ResourceExhausted
            | tonic::Code::Internal
    );
    let error = rpc::decode_status(&status).unwrap_or_else(|| kanban_protocol::ErrorBody {
        code: if status.code() == tonic::Code::InvalidArgument {
            ApiErrorCode::InvalidInput
        } else {
            ApiErrorCode::ServerUnavailable
        },
        message: status.message().to_owned(),
    });
    let detail = pb::ErrorDetail {
        code: pb::DtoApiErrorCode::try_from(error.code)
            .expect("封闭业务错误映射")
            .into(),
        message: error.message,
        retryable,
    };
    Body::Failure(pb::QueryFailure {
        error: Some(detail),
        retryable,
    })
}
