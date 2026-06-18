// Keep this module as a thin rusqlite convenience layer. SQL shape and domain
// semantics stay in the service modules; helpers here only centralize repeated
// binding, row-cardinality, and error-handling plumbing.
use super::storage;

use kanban_core::{KanbanError, Result};

use std::collections::BTreeSet;

use rusqlite::{
    Connection, OptionalExtension, Params, Row, Statement, ToSql, params_from_iter, types::Value,
};

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
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let expected = stmt.parameter_count();
    if expected != params.len() {
        return Err(KanbanError::Storage(format!(
            "expected {expected} SQL parameters, got {}",
            params.len()
        )));
    }
    let rows = stmt
        .query_map(params_from_iter(params.iter()), mapper)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
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

pub(crate) fn required_row<T, P, F, E>(
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

pub(crate) fn scalar<T, P, F>(conn: &Connection, sql: &str, params: P, mapper: F) -> Result<T>
where
    P: Params,
    F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
{
    conn.query_row(sql, params, mapper).map_err(storage)
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

pub(crate) type NamedParams<'a> = &'a [(&'a str, &'a dyn ToSql)];

pub(crate) fn exec_named(conn: &Connection, sql: &str, params: NamedParams<'_>) -> Result<usize> {
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    check_named_params(&stmt, params)?;
    stmt.execute(params).map_err(storage)
}

fn check_named_params(stmt: &Statement<'_>, params: NamedParams<'_>) -> Result<()> {
    let provided = params
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    let mut required = BTreeSet::new();
    for index in 1..=stmt.parameter_count() {
        let Some(name) = stmt
            .parameter_name(index)
            .filter(|name| is_named_parameter(name))
        else {
            return Err(KanbanError::Storage(format!(
                "expected named SQL parameter at index {index}"
            )));
        };
        required.insert(name);
    }
    if let Some(name) = required.difference(&provided).next() {
        return Err(KanbanError::Storage(format!(
            "missing SQL parameter binding: {name}"
        )));
    }
    if let Some(name) = provided.difference(&required).next() {
        return Err(KanbanError::Storage(format!(
            "unused SQL parameter binding: {name}"
        )));
    }
    Ok(())
}

fn is_named_parameter(name: &str) -> bool {
    matches!(name.as_bytes().first(), Some(b':' | b'@' | b'$'))
}

pub(crate) fn ensure_changed_one<E>(changed: usize, err: E) -> Result<()>
where
    E: FnOnce() -> KanbanError,
{
    match changed {
        1 => Ok(()),
        0 => Err(err()),
        n => Err(KanbanError::Storage(format!(
            "expected exactly one affected row, got {n}"
        ))),
    }
}

pub(crate) fn exec_one<P, E>(conn: &Connection, sql: &str, params: P, err: E) -> Result<()>
where
    P: Params,
    E: FnOnce() -> KanbanError,
{
    ensure_changed_one(exec(conn, sql, params)?, err)
}

pub(crate) fn exec_one_named<E>(
    conn: &Connection,
    sql: &str,
    params: NamedParams<'_>,
    err: E,
) -> Result<()>
where
    E: FnOnce() -> KanbanError,
{
    ensure_changed_one(exec_named(conn, sql, params)?, err)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SqlFilter {
    conditions: Vec<&'static str>,
    params: Vec<Value>,
}

impl SqlFilter {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn and(&mut self, condition: &'static str, value: impl IntoSqlValue) -> Result<()> {
        self.and_values(condition, [value])
    }

    pub(crate) fn and_values<I, V>(&mut self, condition: &'static str, values: I) -> Result<()>
    where
        I: IntoIterator<Item = V>,
        V: IntoSqlValue,
    {
        let values = values
            .into_iter()
            .map(IntoSqlValue::into_sql_value)
            .collect::<Vec<_>>();
        let expected = anonymous_parameter_count(condition);
        if expected != values.len() {
            return Err(KanbanError::Storage(format!(
                "condition `{condition}` expected {expected} SQL parameters, got {}",
                values.len()
            )));
        }
        self.conditions.push(condition);
        self.params.extend(values);
        Ok(())
    }

    pub(crate) fn where_sql(&self) -> String {
        if self.conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", self.conditions.join(" AND "))
        }
    }

    pub(crate) fn params(&self) -> &[Value] {
        &self.params
    }
}

fn anonymous_parameter_count(condition: &str) -> usize {
    condition
        .as_bytes()
        .iter()
        .filter(|byte| **byte == b'?')
        .count()
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
    use rusqlite::{Connection, named_params};

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

        let required_name = required_row(
            &conn,
            "SELECT name FROM items WHERE id=?1",
            [2],
            |row| row.get::<_, String>(0),
            || kanban_core::KanbanError::NotFound("item".into()),
        )
        .unwrap();
        assert_eq!(required_name, "two");

        let count: i64 = scalar(&conn, "SELECT COUNT(*) FROM items", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 2);

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
        conn.execute("INSERT INTO items(id, name) VALUES (2, 'two')", [])
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

        let err = exec_one(&conn, "UPDATE items SET name=?1", ["changed"], || {
            kanban_core::KanbanError::InvalidTransition("expected one row".into())
        })
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("expected exactly one affected row, got 2")
        );
    }

    #[test]
    fn sql_filter_builds_where_clause_and_keeps_params_attached_to_conditions() {
        let mut filter = SqlFilter::new();
        assert_eq!(filter.where_sql(), "");
        assert!(filter.params().is_empty());

        filter.and("board_id=?", "b_1").unwrap();
        filter.and("task_id=?", "t_1").unwrap();
        filter.and("status=?", "open").unwrap();

        assert_eq!(
            filter.where_sql(),
            "WHERE board_id=? AND task_id=? AND status=?"
        );
        assert_eq!(filter.params().len(), 3);
    }

    #[test]
    fn sql_filter_supports_conditions_with_multiple_values() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, board_id TEXT NOT NULL, task_id TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO items(id, board_id, task_id) VALUES (1, 'b_1', 't_1'), (2, 'b_1', 't_2')",
            [],
        )
        .unwrap();

        let mut filter = SqlFilter::new();
        filter.and("board_id=?", "b_1").unwrap();
        filter
            .and_values("task_id IN (?, ?)", ["t_1", "t_2"])
            .unwrap();
        let sql = format!(
            "SELECT id FROM items {} ORDER BY id ASC",
            filter.where_sql()
        );

        let ids = all_values(&conn, &sql, filter.params(), |row| row.get::<_, i64>(0)).unwrap();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn all_values_rejects_parameter_count_mismatch() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, board_id TEXT NOT NULL)",
            [],
        )
        .unwrap();

        let too_few = all_values(
            &conn,
            "SELECT id FROM items WHERE board_id=? AND id>?",
            &[Value::Text("b_1".to_owned())],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_err();
        assert!(
            too_few
                .to_string()
                .contains("expected 2 SQL parameters, got 1")
        );

        let too_many = all_values(
            &conn,
            "SELECT id FROM items WHERE board_id=?",
            &[Value::Text("b_1".to_owned()), Value::Integer(1)],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_err();
        assert!(
            too_many
                .to_string()
                .contains("expected 1 SQL parameters, got 2")
        );
    }

    #[test]
    fn exec_named_checks_missing_unused_and_positional_parameters() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();

        let id = 1_i64;
        let missing_name = exec_named(
            &conn,
            "INSERT INTO items(id, name) VALUES (:id, :name)",
            &[(":id", &id as &dyn rusqlite::ToSql)],
        )
        .unwrap_err();
        assert!(
            missing_name
                .to_string()
                .contains("missing SQL parameter binding: :name")
        );

        let unused = exec_named(
            &conn,
            "INSERT INTO items(id, name) VALUES (:id, :name)",
            named_params! {
                ":id": 1_i64,
                ":name": "one",
                ":unused": "ignored",
            },
        )
        .unwrap_err();
        assert!(
            unused
                .to_string()
                .contains("unused SQL parameter binding: :unused")
        );

        let positional = exec_named(
            &conn,
            "INSERT INTO items(id, name) VALUES (?1, :name)",
            named_params! {
                ":name": "one",
            },
        )
        .unwrap_err();
        assert!(
            positional
                .to_string()
                .contains("expected named SQL parameter at index 1")
        );

        exec_named(
            &conn,
            "INSERT INTO items(id, name) VALUES (:id, :name)",
            named_params! {
                ":id": 1_i64,
                ":name": "one",
            },
        )
        .unwrap();
    }
}
