use kanban_core::Clock;

#[derive(Clone, Copy)]
pub(crate) struct FixedClock(pub(crate) i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}
