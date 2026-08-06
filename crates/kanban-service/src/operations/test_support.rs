use std::sync::{Arc, atomic::AtomicUsize};

use kanban_core::Clock;

use crate::*;

#[derive(Clone)]
pub(crate) struct StubStore {
    pub(crate) calls: Arc<AtomicUsize>,
}

impl ApplicationStore for StubStore {}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock(pub(crate) i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}
