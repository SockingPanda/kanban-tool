#![doc = include_str!("../README.md")]
pub mod v1 {
    tonic::include_proto!("kanban.framework.v1");
}
pub const DESCRIPTOR: &[u8] = tonic::include_file_descriptor_set!("kanban");
pub const PROTOCOL_VERSION: u32 = 1;
