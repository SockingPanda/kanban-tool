//! Application storage and runtime ports.
//!
//! This module is intentionally small in the scaffold slice. Concrete ports are
//! added as orchestration moves out of `kanban-sqlite`.

/// Marker trait for storage implementations that back application use cases.
pub trait ApplicationStore {}
