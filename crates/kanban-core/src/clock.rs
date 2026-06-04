use time::OffsetDateTime;

pub trait Clock {
    fn now_ms(&self) -> i64;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000
    }
}
