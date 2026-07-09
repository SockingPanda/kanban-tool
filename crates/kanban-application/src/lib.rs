//! Application-layer contracts for kanban-tool.
//!
//! This crate owns adapter-facing DTOs, ports, and use-case API contracts. Storage
//! crates such as `kanban-sqlite` implement these contracts; adapters should move
//! toward this crate rather than depending on storage implementation exports.

pub mod api;
pub mod dto;
pub mod ports;
