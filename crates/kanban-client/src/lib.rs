//! Thin synchronous client for the canonical localhost kanban host.

mod client;
mod error;
mod operations;
mod selectors;
mod shared;
mod transport;

pub use error::ClientError;
pub use operations::EntityUpsertRequest;
pub use operations::attachment::DownloadedAttachment;

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8721";

#[derive(Clone)]
pub struct KanbanClient {
    pub(crate) base_url: String,
    pub(crate) actor: String,
    pub(crate) agent: ureq::Agent,
}
