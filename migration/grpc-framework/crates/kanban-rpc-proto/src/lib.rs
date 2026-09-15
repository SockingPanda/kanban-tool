//! 框架试验契约。业务扩展应新增命名 RPC，不添加 JSON Execute 通道。
pub mod v1 {
    tonic::include_proto!("kanban.framework.v1");
}
pub const DESCRIPTOR: &[u8] = tonic::include_file_descriptor_set!("kanban");
pub const PROTOCOL_VERSION: u32 = 1;
