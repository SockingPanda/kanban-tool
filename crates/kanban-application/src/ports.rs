/// Marker bound shared by every application capability.
///
/// Persistence methods live on the narrow operation capability traits rather
/// than on this common bound.  The concrete Turso implementation is adapted
/// inside `kanban-server`, which keeps the storage crate out of every other
/// product adapter.
pub trait ApplicationStore: Clone + Send + Sync + 'static {}
