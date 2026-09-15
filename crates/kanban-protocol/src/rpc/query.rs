//! 完整查询流的版本与编码预算；不会把附件下载的消息上限套到查询投影。

pub const PROTOCOL_VERSION: u32 = 1;
pub const PROJECTION_VERSION: u32 = 1;
pub const MAX_QUERIES_PER_STREAM: usize = 64;
pub const MAX_QUERY_FRAME_BYTES: usize = 64 * 1024;
pub const QUERY_CHUNK_BYTES: usize = 60 * 1024;
pub const MAX_QUERY_RESULT_BYTES: usize = 64 * 1024 * 1024;
