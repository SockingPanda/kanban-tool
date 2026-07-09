//! Application-layer contracts for selected kanban-tool use cases.
//!
//! This crate owns adapter-facing DTOs, ports, and use-case API contracts for the
//! vertical slices that have been intentionally pulled across the application
//! boundary. It is not a complete application service layer, and it does not own
//! SQLite transactions, state-machine guards, canonical writes, events, runs, or
//! provenance.
//!
//! Storage crates such as `kanban-sqlite` implement these contracts while
//! retaining transaction ownership in their service layer. Adapters should move
//! toward these DTO/port contracts as each use case is selected, rather than
//! depending on storage implementation exports by default.
//!
//! Evolution policy: add DTO fields as optional or otherwise backward-compatible
//! data first, keep trait changes source-compatible when practical by adding a
//! narrow extension trait or a new selected use-case method, and only make
//! breaking DTO/trait changes together with adapter updates and compile-time
//! contract tests.

pub mod api;
pub mod dto;
pub mod ports;
