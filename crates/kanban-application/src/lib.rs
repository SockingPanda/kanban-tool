//! Shared application-service boundary for every kanban-tool adapter.
//!
//! The HTTP host constructs one [`ApplicationService`] over the canonical
//! store. CLI, MCP and Desktop never construct this service and never receive a
//! storage handle; they reach it through the localhost API.

pub mod dto;
pub mod operations;
pub mod ports;
pub mod service;

pub use dto::*;
pub use ports::ApplicationStore;
pub use service::ApplicationService;
