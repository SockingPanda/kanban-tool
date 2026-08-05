use turso::{Connection, transaction::TransactionBehavior};

use crate::{
    db::TursoStore,
    domain::EntityRecord,
    error::StoreError,
    shared::{
        Value, first_row, integer_value, now_ms, optional_integer_value, optional_text_value,
        text_value,
    },
};

const MAX_ENTITY_LIST_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntityListOptions {
    pub board: Option<String>,
    pub kind: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUpsertInput {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    pub board: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub archived_at: Option<i64>,
}

impl TursoStore {
    pub async fn list_entities(
        &self,
        options: EntityListOptions,
    ) -> Result<Vec<EntityRecord>, StoreError> {
        let limit = normalize_limit(options.limit)?;
        let connection = self.connection().await?;
        let board_id = match options.board.as_deref().map(str::trim) {
            Some(board) if !board.is_empty() => Some(resolve_board_id(&connection, board).await?),
            _ => None,
        };
        let mut params = Vec::<(String, Value)>::new();
        let mut predicates = Vec::new();
        if let Some(board_id) = board_id.as_deref() {
            predicates.push("board_id = :board_id".to_owned());
            params.push((":board_id".to_owned(), Value::Text(board_id.to_owned())));
        }
        if let Some(kind) = options
            .kind
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            predicates.push("kind = :kind".to_owned());
            params.push((":kind".to_owned(), Value::Text(kind.to_owned())));
        }
        params.push((":limit".to_owned(), Value::Integer(limit as i64)));
        let where_sql = if predicates.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", predicates.join(" AND "))
        };
        let mut rows = connection
            .query(
                &format!(
                    "SELECT uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at FROM entities{where_sql} ORDER BY updated_at DESC, uri ASC LIMIT :limit"
                ),
                params,
            )
            .await?;
        let mut entities = Vec::new();
        while let Some(row) = rows.next().await? {
            entities.push(entity_from_row(row)?);
        }
        Ok(entities)
    }

    pub async fn get_entity(&self, uri: &str) -> Result<EntityRecord, StoreError> {
        validate_uri(uri)?;
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at FROM entities WHERE uri = :uri LIMIT 1",
                    [(":uri", uri.trim())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::EntityNotFound(uri.to_owned()),
            other => StoreError::Turso(other),
        })?;
        entity_from_row(row)
    }

    /// Insert or update one canonical entity without replacing its relation
    /// facts.  The `(source_table, source_id)` uniqueness guard is preserved by
    /// the database and is surfaced as an entity conflict.
    pub async fn upsert_entity(
        &self,
        input: EntityUpsertInput,
    ) -> Result<EntityRecord, StoreError> {
        validate_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let mut board_id = match input.board.as_deref().map(str::trim) {
            Some(board) if !board.is_empty() => {
                Some(resolve_board_id_tx(&transaction, board).await?)
            }
            _ => None,
        };
        if let Some(task_id) = input.task_id.as_deref() {
            let task_board = first_row(
                transaction
                    .query(
                        "SELECT board_id FROM tasks WHERE id = :task_id LIMIT 1",
                        [(":task_id", task_id)],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
                other => StoreError::Turso(other),
            })?;
            let task_board = text_value(task_board.get_value(0)?, "tasks.board_id")?;
            if let Some(board_id) = board_id.as_deref() {
                if board_id != task_board {
                    return Err(StoreError::InvalidInput(
                        "entity board and task board must match".to_owned(),
                    ));
                }
            } else {
                board_id = Some(task_board);
            }
        }
        let now = now_ms();
        let insert_params = vec![
            (":uri".to_owned(), Value::Text(input.uri.clone())),
            (
                ":kind".to_owned(),
                Value::Text(input.kind.trim().to_owned()),
            ),
            (
                ":source_table".to_owned(),
                Value::Text(input.source_table.trim().to_owned()),
            ),
            (
                ":source_id".to_owned(),
                Value::Text(input.source_id.trim().to_owned()),
            ),
            (
                ":board_id".to_owned(),
                board_id
                    .as_deref()
                    .map_or(Value::Null, |v| Value::Text(v.to_owned())),
            ),
            (
                ":task_id".to_owned(),
                input
                    .task_id
                    .as_deref()
                    .map_or(Value::Null, |v| Value::Text(v.to_owned())),
            ),
            (
                ":title".to_owned(),
                input
                    .title
                    .as_deref()
                    .map_or(Value::Null, |v| Value::Text(v.to_owned())),
            ),
            (
                ":summary".to_owned(),
                input
                    .summary
                    .as_deref()
                    .map_or(Value::Null, |v| Value::Text(v.to_owned())),
            ),
            (
                ":content_hash".to_owned(),
                input
                    .content_hash
                    .as_deref()
                    .map_or(Value::Null, |v| Value::Text(v.to_owned())),
            ),
            (":created_at".to_owned(), Value::Integer(now)),
            (":updated_at".to_owned(), Value::Integer(now)),
            (
                ":archived_at".to_owned(),
                input.archived_at.map_or(Value::Null, Value::Integer),
            ),
        ];
        let result = transaction
            .execute(
                "INSERT INTO entities(uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at) VALUES (:uri, :kind, :source_table, :source_id, :board_id, :task_id, :title, :summary, :content_hash, :created_at, :updated_at, :archived_at) ON CONFLICT(uri) DO UPDATE SET kind = excluded.kind, source_table = excluded.source_table, source_id = excluded.source_id, board_id = excluded.board_id, task_id = excluded.task_id, title = excluded.title, summary = excluded.summary, content_hash = excluded.content_hash, updated_at = excluded.updated_at, archived_at = excluded.archived_at",
                insert_params,
            )
            .await;
        if let Err(error) = result {
            if matches!(error, turso::Error::Constraint(_)) {
                return Err(StoreError::EntityConflict(error.to_string()));
            }
            return Err(StoreError::Turso(error));
        }
        let row = first_row(
            transaction
                .query(
                    "SELECT uri, kind, source_table, source_id, board_id, task_id, title, summary, content_hash, created_at, updated_at, archived_at FROM entities WHERE uri = :uri LIMIT 1",
                    [(":uri", input.uri.as_str())],
                )
                .await?,
        )
        .await?;
        let entity = entity_from_row(row)?;
        transaction.commit().await?;
        Ok(entity)
    }
}

fn normalize_limit(limit: usize) -> Result<usize, StoreError> {
    if limit == 0 || limit > MAX_ENTITY_LIST_LIMIT {
        return Err(StoreError::InvalidInput(format!(
            "entity limit must be between 1 and {MAX_ENTITY_LIST_LIMIT}"
        )));
    }
    Ok(limit)
}

fn validate_uri(uri: &str) -> Result<(), StoreError> {
    let uri = uri.trim();
    if !uri.starts_with("kb://") || uri.len() <= "kb://".len() {
        return Err(StoreError::InvalidInput(
            "entity uri must start with kb://".to_owned(),
        ));
    }
    Ok(())
}

fn validate_input(input: &EntityUpsertInput) -> Result<(), StoreError> {
    validate_uri(&input.uri)?;
    for (name, value) in [
        ("kind", input.kind.as_str()),
        ("source_table", input.source_table.as_str()),
        ("source_id", input.source_id.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(StoreError::InvalidInput(format!(
                "entity {name} is required"
            )));
        }
    }
    if let Some(task_id) = input.task_id.as_deref() {
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "entity task_id must start with t_".to_owned(),
            ));
        }
    }
    Ok(())
}

async fn resolve_board_id(connection: &Connection, selector: &str) -> Result<String, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT id FROM boards WHERE id = :selector OR slug = :selector LIMIT 1",
                [(":selector", selector)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

async fn resolve_board_id_tx(
    transaction: &turso::transaction::Transaction<'_>,
    selector: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id FROM boards WHERE id = :selector OR slug = :selector LIMIT 1",
                [(":selector", selector)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

fn entity_from_row(row: turso::Row) -> Result<EntityRecord, StoreError> {
    Ok(EntityRecord {
        uri: text_value(row.get_value(0)?, "entities.uri")?,
        kind: text_value(row.get_value(1)?, "entities.kind")?,
        source_table: text_value(row.get_value(2)?, "entities.source_table")?,
        source_id: text_value(row.get_value(3)?, "entities.source_id")?,
        board_id: optional_text_value(row.get_value(4)?, "entities.board_id")?,
        task_id: optional_text_value(row.get_value(5)?, "entities.task_id")?,
        title: optional_text_value(row.get_value(6)?, "entities.title")?,
        summary: optional_text_value(row.get_value(7)?, "entities.summary")?,
        content_hash: optional_text_value(row.get_value(8)?, "entities.content_hash")?,
        created_at: integer_value(row.get_value(9)?, "entities.created_at")?,
        updated_at: integer_value(row.get_value(10)?, "entities.updated_at")?,
        archived_at: optional_integer_value(row.get_value(11)?, "entities.archived_at")?,
    })
}
