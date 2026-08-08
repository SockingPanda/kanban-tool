#![doc = include_str!("../README.md")]

//! 面向 canonical localhost kanban host 的轻量同步客户端。

mod client;
mod error;
mod operations;
mod selectors;
mod shared;
mod transport;

pub use error::ClientError;
pub use operations::EntityUpsertRequest;
pub use operations::attachment::DownloadedAttachment;
pub use operations::{EventStream, EventStreamItem};

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8721";

#[derive(Clone)]
pub struct KanbanClient {
    pub(crate) base_url: String,
    pub(crate) actor: String,
    pub(crate) agent: ureq::Agent,
}
