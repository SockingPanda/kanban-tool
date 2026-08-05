use std::sync::Arc;

use kanban_core::{Clock, SystemClock};
use tokio::sync::Mutex;

use crate::ApplicationStore;

/// The canonical command/query entry point shared by the HTTP handlers and the
/// in-process dispatcher.
#[derive(Debug, Clone)]
pub struct ApplicationService<S, C = SystemClock> {
    pub(crate) store: S,
    pub(crate) clock: C,
    pub(crate) mutation_gate: Arc<Mutex<()>>,
}

impl<S> ApplicationService<S, SystemClock>
where
    S: ApplicationStore,
{
    pub fn new(store: S) -> Self {
        Self {
            store,
            clock: SystemClock,
            mutation_gate: Arc::new(Mutex::new(())),
        }
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub fn with_clock(store: S, clock: C) -> Self {
        Self {
            store,
            clock,
            mutation_gate: Arc::new(Mutex::new(())),
        }
    }
}
