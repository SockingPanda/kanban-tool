//! Explicit public facade for SQLite-backed application use cases.
//!
//! `service` remains the implementation owner for transactions, state-machine
//! guards, canonical writes, events, runs, and provenance. Adapters should use
//! this module instead of relying on crate-root legacy re-exports.

pub use crate::service::*;
