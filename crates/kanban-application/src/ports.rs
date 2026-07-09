//! Application storage and runtime ports.
//!
//! This module is intentionally small because `kanban-application` is currently a
//! selected vertical slice, not a full application-service crate. Concrete ports
//! are added only when a use case is intentionally moved behind this boundary;
//! SQLite transaction ownership remains in `kanban-sqlite::service` until a
//! future extraction has a concrete, tested reason.

/// Marker trait for storage implementations that back application use cases.
pub trait ApplicationStore {}
