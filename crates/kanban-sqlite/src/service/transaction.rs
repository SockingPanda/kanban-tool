use kanban_core::{KanbanError, Result};
use rusqlite::Connection;

pub(super) fn with_immediate_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    with_tx(conn, "BEGIN IMMEDIATE", f)
}

pub(super) fn with_read_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    with_tx(conn, "BEGIN", f)
}

fn with_tx<T>(conn: &Connection, begin_sql: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let mut tx = TxGuard::begin(conn, begin_sql)?;
    match f() {
        Ok(value) => {
            tx.commit()?;
            Ok(value)
        }
        Err(err) => {
            if let Err(rollback_err) = tx.rollback() {
                Err(KanbanError::Storage(format!(
                    "rollback failed after transaction error: {rollback_err}; original error: {err}"
                )))
            } else {
                Err(err)
            }
        }
    }
}

struct TxGuard<'conn> {
    conn: &'conn Connection,
    active: bool,
}

impl<'conn> TxGuard<'conn> {
    fn begin(conn: &'conn Connection, begin_sql: &str) -> Result<Self> {
        conn.execute_batch(begin_sql).map_err(storage)?;
        Ok(Self { conn, active: true })
    }

    fn commit(&mut self) -> Result<()> {
        self.conn.execute_batch("COMMIT").map_err(storage)?;
        self.active = false;
        Ok(())
    }

    fn rollback(&mut self) -> rusqlite::Result<()> {
        if self.active {
            self.conn.execute_batch("ROLLBACK")?;
            self.active = false;
        }
        Ok(())
    }
}

impl Drop for TxGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.conn.execute_batch("ROLLBACK");
            self.active = false;
        }
    }
}

pub(super) fn storage(err: rusqlite::Error) -> KanbanError {
    KanbanError::Storage(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::panic::{self, AssertUnwindSafe};

    use super::*;

    #[test]
    fn immediate_tx_rolls_back_when_closure_panics() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();

        let panic_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = with_immediate_tx(&conn, || -> Result<()> {
                conn.execute("INSERT INTO items(id, name) VALUES (1, 'one')", [])
                    .map_err(storage)?;
                panic!("forced transaction panic");
            });
        }));

        assert!(panic_result.is_err());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn immediate_tx_reports_rollback_failure_after_error() {
        let conn = Connection::open_in_memory().unwrap();
        let err = with_immediate_tx(&conn, || -> Result<()> {
            conn.execute_batch("COMMIT").map_err(storage)?;
            Err(KanbanError::InvalidInput("closure failed".into()))
        })
        .unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("rollback failed after transaction error"),
            "err: {message}"
        );
        assert!(message.contains("closure failed"), "err: {message}");
    }
}
