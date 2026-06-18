use super::storage;

use kanban_core::{KanbanError, Result};

use rusqlite::{Connection, OptionalExtension, Params, Row, params_from_iter, types::Value};

pub(crate) fn all<T, P, F>(conn: &Connection, sql: &str, params: P, mapper: F) -> Result<Vec<T>>
where
    P: Params,
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let rows = stmt.query_map(params, mapper).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub(crate) fn all_values<T, F>(
    conn: &Connection,
    sql: &str,
    params: &[Value],
    mapper: F,
) -> Result<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    all(conn, sql, params_from_iter(params.iter()), mapper)
}

pub(crate) fn optional<T, P, F>(
    conn: &Connection,
    sql: &str,
    params: P,
    mapper: F,
) -> Result<Option<T>>
where
    P: Params,
    F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
{
    conn.query_row(sql, params, mapper)
        .optional()
        .map_err(storage)
}

pub(crate) fn one<T, P, F, E>(
    conn: &Connection,
    sql: &str,
    params: P,
    mapper: F,
    missing: E,
) -> Result<T>
where
    P: Params,
    F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    E: FnOnce() -> KanbanError,
{
    optional(conn, sql, params, mapper)?.ok_or_else(missing)
}

pub(crate) fn exists<P>(conn: &Connection, sql: &str, params: P) -> Result<bool>
where
    P: Params,
{
    optional(conn, sql, params, |_row| Ok(())).map(|row| row.is_some())
}

pub(crate) fn exec<P>(conn: &Connection, sql: &str, params: P) -> Result<usize>
where
    P: Params,
{
    conn.execute(sql, params).map_err(storage)
}

pub(crate) fn ensure_changed_one<E>(changed: usize, err: E) -> Result<()>
where
    E: FnOnce() -> KanbanError,
{
    if changed == 1 { Ok(()) } else { Err(err()) }
}

pub(crate) fn exec_one<P, E>(conn: &Connection, sql: &str, params: P, err: E) -> Result<()>
where
    P: Params,
    E: FnOnce() -> KanbanError,
{
    ensure_changed_one(exec(conn, sql, params)?, err)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SqlWhere {
    sql: String,
    params: Vec<Value>,
}

impl SqlWhere {
    pub(crate) fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            params: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, condition: &str, value: impl IntoSqlValue) {
        if !self.sql.is_empty() {
            self.sql.push(' ');
        }
        self.sql.push_str(condition);
        self.push_value(value);
    }

    pub(crate) fn push_value(&mut self, value: impl IntoSqlValue) {
        self.params.push(value.into_sql_value());
    }

    pub(crate) fn sql(&self) -> &str {
        &self.sql
    }

    pub(crate) fn params(&self) -> &[Value] {
        &self.params
    }
}

pub(crate) trait IntoSqlValue {
    fn into_sql_value(self) -> Value;
}

impl IntoSqlValue for Value {
    fn into_sql_value(self) -> Value {
        self
    }
}

impl IntoSqlValue for &str {
    fn into_sql_value(self) -> Value {
        Value::Text(self.to_owned())
    }
}

impl IntoSqlValue for String {
    fn into_sql_value(self) -> Value {
        Value::Text(self)
    }
}

impl IntoSqlValue for &String {
    fn into_sql_value(self) -> Value {
        Value::Text(self.clone())
    }
}

impl IntoSqlValue for i64 {
    fn into_sql_value(self) -> Value {
        Value::Integer(self)
    }
}

impl IntoSqlValue for usize {
    fn into_sql_value(self) -> Value {
        Value::Integer(self.try_into().expect("usize SQL value overflows i64"))
    }
}

impl IntoSqlValue for Option<i64> {
    fn into_sql_value(self) -> Value {
        self.map_or(Value::Null, Value::Integer)
    }
}

impl IntoSqlValue for Option<String> {
    fn into_sql_value(self) -> Value {
        self.map_or(Value::Null, Value::Text)
    }
}

impl IntoSqlValue for Option<&str> {
    fn into_sql_value(self) -> Value {
        self.map_or(Value::Null, |value| Value::Text(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn query_helpers_map_common_rusqlite_shapes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();

        exec(
            &conn,
            "INSERT INTO items(id, name) VALUES (?1, ?2)",
            (1, "one"),
        )
        .unwrap();
        exec(
            &conn,
            "INSERT INTO items(id, name) VALUES (?1, ?2)",
            (2, "two"),
        )
        .unwrap();

        let names = all(&conn, "SELECT name FROM items ORDER BY id ASC", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap();
        assert_eq!(names, vec!["one", "two"]);

        let present = optional(&conn, "SELECT name FROM items WHERE id=?1", [1], |row| {
            row.get::<_, String>(0)
        })
        .unwrap();
        assert_eq!(present.as_deref(), Some("one"));

        let missing = optional(&conn, "SELECT name FROM items WHERE id=?1", [99], |row| {
            row.get::<_, String>(0)
        })
        .unwrap();
        assert!(missing.is_none());

        let one_name = one(
            &conn,
            "SELECT name FROM items WHERE id=?1",
            [2],
            |row| row.get::<_, String>(0),
            || kanban_core::KanbanError::NotFound("item".into()),
        )
        .unwrap();
        assert_eq!(one_name, "two");

        assert!(exists(&conn, "SELECT 1 FROM items WHERE id=?1", [1]).unwrap());
        assert!(!exists(&conn, "SELECT 1 FROM items WHERE id=?1", [99]).unwrap());
    }

    #[test]
    fn exec_one_requires_exactly_one_changed_row() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO items(id, name) VALUES (1, 'one')", [])
            .unwrap();

        exec_one(
            &conn,
            "UPDATE items SET name=?1 WHERE id=?2",
            ("uno", 1),
            || kanban_core::KanbanError::InvalidTransition("expected one row".into()),
        )
        .unwrap();

        let err = exec_one(
            &conn,
            "UPDATE items SET name=?1 WHERE id=?2",
            ("missing", 99),
            || kanban_core::KanbanError::InvalidTransition("expected one row".into()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("expected one row"));
    }

    #[test]
    fn sql_where_accumulates_anonymous_conditions_and_params() {
        let mut where_clause = SqlWhere::new("WHERE board_id=?");
        where_clause.push_value("b_1");
        where_clause.push("AND task_id=?", "t_1");
        where_clause.push("AND status=?", "open");

        assert_eq!(
            where_clause.sql(),
            "WHERE board_id=? AND task_id=? AND status=?"
        );
        assert_eq!(where_clause.params().len(), 3);
    }
}
