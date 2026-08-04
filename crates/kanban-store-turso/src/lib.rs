mod schema;

use std::{
    error::Error,
    fmt::{Display, Formatter},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use turso::{Builder, Connection, Database, Row, Rows, Value, transaction::TransactionBehavior};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardColumnRecord {
    pub id: String,
    pub board_id: String,
    pub status: String,
    pub title: String,
    pub position: i64,
    pub hidden: bool,
    pub wip_limit: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug)]
pub enum StoreError {
    Turso(turso::Error),
    InvalidPath,
    InvalidStoredValue { field: &'static str },
    BoardNotFound(String),
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Turso(error) => write!(formatter, "turso error: {error}"),
            Self::InvalidPath => write!(formatter, "database path must be valid non-empty UTF-8"),
            Self::InvalidStoredValue { field } => {
                write!(formatter, "invalid stored value for {field}")
            }
            Self::BoardNotFound(selector) => write!(formatter, "board not found: {selector}"),
        }
    }
}

impl Error for StoreError {}

impl From<turso::Error> for StoreError {
    fn from(error: turso::Error) -> Self {
        Self::Turso(error)
    }
}

#[derive(Clone)]
pub struct TursoStore {
    database: Database,
}

impl TursoStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_str().ok_or(StoreError::InvalidPath)?;
        if path.is_empty() {
            return Err(StoreError::InvalidPath);
        }
        let database = Builder::new_local(path).build().await?;
        Ok(Self { database })
    }

    pub async fn initialize(&self) -> Result<(), StoreError> {
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        transaction.execute_batch(schema::CANONICAL_SCHEMA).await?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at) VALUES (?1, ?2, '', ?3)",
                (schema::SCHEMA_VERSION, schema::SCHEMA_NAME, now_ms()),
            )
            .await?;

        transaction
            .execute(
                "INSERT OR IGNORE INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_default', 'default', 'Default', NULL, ?1, ?1, NULL)",
                [now_ms()],
            )
            .await?;
        let board_id = first_row(
            transaction
                .query("SELECT id FROM boards WHERE slug = 'default'", ())
                .await?,
        )
        .await?
        .get_value(0)
        .map_err(StoreError::from)
        .and_then(|value| text_value(value, "boards.id"))?;

        for (status, title, position, hidden) in schema::DEFAULT_COLUMNS {
            let id = format!("col_{}_{}", board_id.trim_start_matches("b_"), status);
            transaction
                .execute(
                    "INSERT OR IGNORE INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
                    (id, board_id.as_str(), status, title, position, hidden, now_ms()),
                )
                .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn list_boards(
        &self,
        include_archived: bool,
    ) -> Result<Vec<BoardRecord>, StoreError> {
        let connection = self.connection().await?;
        let sql = if include_archived {
            "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards ORDER BY archived_at IS NOT NULL ASC, slug ASC, id ASC"
        } else {
            "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards WHERE archived_at IS NULL ORDER BY slug ASC, id ASC"
        };
        let mut rows = connection.query(sql, ()).await?;
        let mut boards = Vec::new();
        while let Some(row) = rows.next().await? {
            boards.push(BoardRecord {
                id: text_value(row.get_value(0)?, "boards.id")?,
                slug: text_value(row.get_value(1)?, "boards.slug")?,
                name: text_value(row.get_value(2)?, "boards.name")?,
                description: optional_text_value(row.get_value(3)?, "boards.description")?,
                created_at: integer_value(row.get_value(4)?, "boards.created_at")?,
                updated_at: integer_value(row.get_value(5)?, "boards.updated_at")?,
                archived_at: optional_integer_value(row.get_value(6)?, "boards.archived_at")?,
            });
        }
        Ok(boards)
    }

    pub async fn list_board_columns(
        &self,
        selector: &str,
    ) -> Result<Vec<BoardColumnRecord>, StoreError> {
        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        let mut rows = connection
            .query(
                "SELECT id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at FROM board_columns WHERE board_id = ?1 ORDER BY position ASC, id ASC",
                [board_id.as_str()],
            )
            .await?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await? {
            columns.push(BoardColumnRecord {
                id: text_value(row.get_value(0)?, "board_columns.id")?,
                board_id: text_value(row.get_value(1)?, "board_columns.board_id")?,
                status: text_value(row.get_value(2)?, "board_columns.status")?,
                title: text_value(row.get_value(3)?, "board_columns.title")?,
                position: integer_value(row.get_value(4)?, "board_columns.position")?,
                hidden: integer_value(row.get_value(5)?, "board_columns.hidden")? != 0,
                wip_limit: optional_integer_value(row.get_value(6)?, "board_columns.wip_limit")?,
                created_at: integer_value(row.get_value(7)?, "board_columns.created_at")?,
                updated_at: integer_value(row.get_value(8)?, "board_columns.updated_at")?,
            });
        }
        Ok(columns)
    }

    async fn connection(&self) -> Result<Connection, StoreError> {
        let connection = self.database.connect()?;
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        Ok(connection)
    }
}

async fn first_row(mut rows: Rows) -> Result<Row, turso::Error> {
    let row = rows
        .next()
        .await?
        .ok_or(turso::Error::QueryReturnedNoRows)?;
    while rows.next().await?.is_some() {}
    Ok(row)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn text_value(value: Value, field: &'static str) -> Result<String, StoreError> {
    match value {
        Value::Text(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn optional_text_value(value: Value, field: &'static str) -> Result<Option<String>, StoreError> {
    match value {
        Value::Text(value) => Ok(Some(value)),
        Value::Null => Ok(None),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn integer_value(value: Value, field: &'static str) -> Result<i64, StoreError> {
    match value {
        Value::Integer(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn optional_integer_value(value: Value, field: &'static str) -> Result<Option<i64>, StoreError> {
    match value {
        Value::Integer(value) => Ok(Some(value)),
        Value::Null => Ok(None),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn store(name: &str) -> (tempfile::TempDir, TursoStore, PathBuf) {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join(format!("{name}.db"));
        let store = TursoStore::open(&path).await.expect("open Turso database");
        (directory, store, path)
    }

    #[tokio::test]
    async fn fresh_database_bootstraps_canonical_tables() {
        let (_directory, store, _path) = store("bootstrap").await;
        store.initialize().await.expect("initialize");

        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '__turso_internal_%' ORDER BY name",
                (),
            )
            .await
            .expect("table query");
        let mut names = Vec::new();
        while let Some(row) = rows.next().await.expect("next table row") {
            names.push(
                text_value(row.get_value(0).expect("table name"), "sqlite_master.name")
                    .expect("text table name"),
            );
        }
        assert_eq!(
            names,
            vec![
                "board_columns",
                "boards",
                "schema_migrations",
                "task_comments",
                "task_dependencies",
                "task_events",
                "task_execution_plans",
                "task_runs",
                "task_steps",
                "tasks",
            ]
        );
    }

    #[tokio::test]
    async fn initialize_is_idempotent_and_seeds_default_board_columns() {
        let (_directory, store, path) = store("idempotent").await;
        store.initialize().await.expect("first initialize");
        store.initialize().await.expect("second initialize");

        let boards = store.list_boards(false).await.expect("list boards");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].slug, "default");

        let columns = store
            .list_board_columns("default")
            .await
            .expect("list columns");
        assert_eq!(columns.len(), 9);
        assert_eq!(
            columns
                .iter()
                .map(|column| (column.status.as_str(), column.position, column.hidden))
                .collect::<Vec<_>>(),
            vec![
                ("triage", 10, false),
                ("todo", 20, false),
                ("scheduled", 30, false),
                ("ready", 40, false),
                ("running", 50, false),
                ("blocked", 60, false),
                ("review", 70, false),
                ("done", 80, false),
                ("archived", 90, true),
            ]
        );

        drop(store);
        let reopened = TursoStore::open(path).await.expect("reopen database");
        reopened.initialize().await.expect("reinitialize database");
        assert_eq!(
            reopened
                .list_boards(false)
                .await
                .expect("list after reopen")
                .len(),
            1
        );
        assert_eq!(
            reopened
                .list_board_columns("b_default")
                .await
                .expect("columns by id")
                .len(),
            9
        );
    }

    #[tokio::test]
    async fn include_archived_filters_and_orders_boards() {
        let (_directory, store, _path) = store("archived").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at, archived_at) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
                ("b_archived", "archive", "Archive", 2_i64, 3_i64),
            )
            .await
            .expect("insert archived board");

        let active = store.list_boards(false).await.expect("active boards");
        assert_eq!(
            active
                .iter()
                .map(|board| board.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["default"]
        );
        let all = store.list_boards(true).await.expect("all boards");
        assert_eq!(
            all.iter()
                .map(|board| board.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["default", "archive"]
        );
    }

    #[tokio::test]
    async fn idempotency_and_board_column_constraints_are_enforced() {
        let (_directory, store, _path) = store("constraints").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'todo', ?6, ?7, ?7)",
                ("t_one", "b_default", 1_i64, "client-1", "One", "test", 1_i64),
            )
            .await
            .expect("insert first task");
        let duplicate_idempotency = connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'todo', ?6, ?7, ?7)",
                ("t_two", "b_default", 2_i64, "client-1", "Two", "test", 2_i64),
            )
            .await;
        assert!(
            duplicate_idempotency.is_err(),
            "task idempotency must be unique per board"
        );

        let duplicate_column = connection
            .execute(
                "INSERT INTO board_columns(id, board_id, status, title, position, hidden, created_at, updated_at) VALUES (?1, ?2, 'todo', 'Duplicate', ?3, 0, ?4, ?4)",
                ("col_duplicate", "b_default", 200_i64, 2_i64),
            )
            .await;
        assert!(
            duplicate_column.is_err(),
            "board status columns must be unique"
        );
    }
}
