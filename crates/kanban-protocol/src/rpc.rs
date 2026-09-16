//! 正式本机 RPC wire contract、精确 DTO codec 与稳定错误细节。

pub mod catalog;
mod codec;
pub mod dto;
mod generated;
pub mod query;

/// Protobuf 生成的消息与 tonic client/server 类型。
pub mod v1 {
    tonic::include_proto!("kanban.v1");
}

/// 正式 RPC descriptor；包含具名业务操作与完整查询流。
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("kanban_v1");

/// 256 MiB 附件及 1 MiB envelope 预算；service 仍执行附件内容的 256 MiB 上限。
pub const MAX_MESSAGE_BYTES: usize = 257 * 1024 * 1024;

pub use codec::{RpcCodecError, decode_json, decode_status, encode_json, encode_status};

/// 操作到原 DTO parts 的生成映射，供 adapter 和覆盖验证消费。
pub const METHOD_MANIFEST: &str = include_str!("../proto/rpc-methods.json");

/// 同一 descriptor 中的对象和文件扩展。命名层级与 Protobuf package 一致。
pub mod extensions {
    pub mod v1 {
        tonic::include_proto!("kanban.extensions.v1");
    }
    pub use v1::*;
}
