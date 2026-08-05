use turso::{Connection, Value, transaction::TransactionBehavior};

use crate::{
    db::TursoStore,
    domain::{RelationPredicateRecord, RelationRecord},
    error::StoreError,
    shared::{
        first_row, integer_value, now_ms, optional_integer_value, optional_text_value, text_value,
    },
};

const MAX_RELATION_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationListOptions {
    pub board: Option<String>,
    pub subject_uri: Option<String>,
    pub object_uri: Option<String>,
    pub predicate: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationPredicateInput {
    pub name: String,
    pub domain_kind: Option<String>,
    pub range_kind: Option<String>,
    pub cardinality: String,
    pub authoritative_store: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationUpsertInput {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub board: Option<String>,
    pub authoritative_store: String,
    pub source_table: Option<String>,
    pub source_id: Option<String>,
    pub source_event_id: Option<i64>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDeleteInput {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub board: Option<String>,
}

impl TursoStore {
    pub async fn list_relation_predicates(
        &self,
    ) -> Result<Vec<RelationPredicateRecord>, StoreError> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT name, domain_kind, range_kind, cardinality, authoritative_store, description, created_at FROM relation_predicates ORDER BY name ASC",
                (),
            )
            .await?;
        let mut predicates = Vec::new();
        while let Some(row) = rows.next().await? {
            predicates.push(predicate_from_row(row)?);
        }
        Ok(predicates)
    }

    pub async fn upsert_relation_predicate(
        &self,
        input: RelationPredicateInput,
    ) -> Result<RelationPredicateRecord, StoreError> {
        validate_predicate_input(&input)?;
        let connection = self.connection().await?;
        let now = now_ms();
        connection
            .execute(
                "INSERT INTO relation_predicates(name, domain_kind, range_kind, cardinality, authoritative_store, description, created_at) VALUES (:name, :domain_kind, :range_kind, :cardinality, :authoritative_store, :description, :created_at) ON CONFLICT(name) DO UPDATE SET domain_kind = excluded.domain_kind, range_kind = excluded.range_kind, cardinality = excluded.cardinality, authoritative_store = excluded.authoritative_store, description = excluded.description",
                vec![
                    (":name".to_owned(), Value::Text(input.name.clone())),
                    (":domain_kind".to_owned(), optional_text(input.domain_kind.as_deref())),
                    (":range_kind".to_owned(), optional_text(input.range_kind.as_deref())),
                    (":cardinality".to_owned(), Value::Text(input.cardinality.clone())),
                    (":authoritative_store".to_owned(), Value::Text(input.authoritative_store.clone())),
                    (":description".to_owned(), optional_text(input.description.as_deref())),
                    (":created_at".to_owned(), Value::Integer(now)),
                ],
            )
            .await?;
        let row = first_row(
            connection
                .query(
                    "SELECT name, domain_kind, range_kind, cardinality, authoritative_store, description, created_at FROM relation_predicates WHERE name = :name LIMIT 1",
                    [(":name", input.name.as_str())],
                )
                .await?,
        )
        .await?;
        predicate_from_row(row)
    }

    pub async fn list_relations(
        &self,
        options: RelationListOptions,
    ) -> Result<Vec<RelationRecord>, StoreError> {
        let limit = normalize_limit(options.limit)?;
        let connection = self.connection().await?;
        let board_id = match options.board.as_deref().map(str::trim) {
            Some(board) if !board.is_empty() => Some(resolve_board_id(&connection, board).await?),
            _ => None,
        };
        let mut predicates = Vec::new();
        let mut params = Vec::<(String, Value)>::new();
        if let Some(board_id) = board_id.as_deref() {
            predicates.push("r.board_id = :board_id".to_owned());
            params.push((":board_id".to_owned(), Value::Text(board_id.to_owned())));
        }
        if let Some(subject) = options.subject_uri.as_deref() {
            validate_uri(subject)?;
            predicates.push("r.subject_uri = :subject_uri".to_owned());
            params.push((":subject_uri".to_owned(), Value::Text(subject.to_owned())));
        }
        if let Some(object) = options.object_uri.as_deref() {
            validate_uri(object)?;
            predicates.push("r.object_uri = :object_uri".to_owned());
            params.push((":object_uri".to_owned(), Value::Text(object.to_owned())));
        }
        if let Some(predicate) = options
            .predicate
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            predicates.push("r.predicate = :predicate".to_owned());
            params.push((":predicate".to_owned(), Value::Text(predicate.to_owned())));
        }
        params.push((":limit".to_owned(), Value::Integer(limit as i64)));
        let where_sql = if predicates.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", predicates.join(" AND "))
        };
        let mut rows = connection
            .query(
                &format!("SELECT r.id, r.subject_uri, r.predicate, r.object_uri, r.graph_uri, r.board_id, r.authoritative_store, r.source_table, r.source_id, r.source_event_id, r.metadata_json, r.created_at, r.updated_at FROM entity_relations r{where_sql} ORDER BY r.id ASC LIMIT :limit"),
                params,
            )
            .await?;
        let mut relations = Vec::new();
        while let Some(row) = rows.next().await? {
            relations.push(relation_from_row(row)?);
        }
        Ok(relations)
    }

    pub async fn upsert_relation(
        &self,
        input: RelationUpsertInput,
    ) -> Result<RelationRecord, StoreError> {
        validate_relation_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let subject_board = entity_board_id(&transaction, &input.subject_uri).await?;
        let object_board = entity_board_id(&transaction, &input.object_uri).await?;
        if subject_board != object_board {
            return Err(StoreError::RelationConflict(
                "relation endpoints must belong to the same board".to_owned(),
            ));
        }
        let board_id = match input.board.as_deref().map(str::trim) {
            Some(board) if !board.is_empty() => {
                let selected = resolve_board_id_tx(&transaction, board).await?;
                if Some(selected.clone()) != subject_board {
                    return Err(StoreError::RelationConflict(
                        "relation board does not match endpoint boards".to_owned(),
                    ));
                }
                Some(selected)
            }
            _ => subject_board,
        };
        let predicate = first_row(
            transaction
                .query(
                    "SELECT name FROM relation_predicates WHERE name = :predicate LIMIT 1",
                    [(":predicate", input.predicate.as_str())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::PredicateNotFound(input.predicate.clone())
            }
            other => StoreError::Turso(other),
        })?;
        let _ = text_value(predicate.get_value(0)?, "relation_predicates.name")?;
        let now = now_ms();
        let result = transaction
            .execute(
                "INSERT INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, board_id, authoritative_store, source_table, source_id, source_event_id, metadata_json, created_at, updated_at) VALUES (:subject_uri, :predicate, :object_uri, :graph_uri, :board_id, :authoritative_store, :source_table, :source_id, :source_event_id, :metadata_json, :created_at, :updated_at) ON CONFLICT(subject_uri, predicate, object_uri, graph_uri) DO UPDATE SET board_id = excluded.board_id, authoritative_store = excluded.authoritative_store, source_table = excluded.source_table, source_id = excluded.source_id, source_event_id = excluded.source_event_id, metadata_json = excluded.metadata_json, updated_at = excluded.updated_at",
                vec![
                    (":subject_uri".to_owned(), Value::Text(input.subject_uri.clone())),
                    (":predicate".to_owned(), Value::Text(input.predicate.clone())),
                    (":object_uri".to_owned(), Value::Text(input.object_uri.clone())),
                    (":graph_uri".to_owned(), Value::Text(input.graph_uri.clone())),
                    (":board_id".to_owned(), optional_text(board_id.as_deref())),
                    (":authoritative_store".to_owned(), Value::Text(input.authoritative_store.clone())),
                    (":source_table".to_owned(), optional_text(input.source_table.as_deref())),
                    (":source_id".to_owned(), optional_text(input.source_id.as_deref())),
                    (":source_event_id".to_owned(), input.source_event_id.map_or(Value::Null, Value::Integer)),
                    (":metadata_json".to_owned(), Value::Text(input.metadata_json.clone())),
                    (":created_at".to_owned(), Value::Integer(now)),
                    (":updated_at".to_owned(), Value::Integer(now)),
                ],
            )
            .await;
        if let Err(error) = result {
            if matches!(error, turso::Error::Constraint(_)) {
                return Err(StoreError::RelationConflict(error.to_string()));
            }
            return Err(StoreError::Turso(error));
        }
        let row = first_row(
            transaction
                .query(
                    "SELECT r.id, r.subject_uri, r.predicate, r.object_uri, r.graph_uri, r.board_id, r.authoritative_store, r.source_table, r.source_id, r.source_event_id, r.metadata_json, r.created_at, r.updated_at FROM entity_relations r WHERE r.subject_uri = :subject_uri AND r.predicate = :predicate AND r.object_uri = :object_uri AND r.graph_uri = :graph_uri LIMIT 1",
                    [
                        (":subject_uri", input.subject_uri.as_str()),
                        (":predicate", input.predicate.as_str()),
                        (":object_uri", input.object_uri.as_str()),
                        (":graph_uri", input.graph_uri.as_str()),
                    ],
                )
                .await?,
        )
        .await?;
        let relation = relation_from_row(row)?;
        transaction.commit().await?;
        Ok(relation)
    }

    pub async fn delete_relation(&self, input: RelationDeleteInput) -> Result<bool, StoreError> {
        validate_uri(&input.subject_uri)?;
        validate_uri(&input.object_uri)?;
        validate_uri(&input.graph_uri)?;
        let connection = self.connection().await?;
        let board_id = match input.board.as_deref().map(str::trim) {
            Some(board) if !board.is_empty() => Some(resolve_board_id(&connection, board).await?),
            _ => None,
        };
        let mut params = vec![
            (":subject_uri".to_owned(), Value::Text(input.subject_uri)),
            (":predicate".to_owned(), Value::Text(input.predicate)),
            (":object_uri".to_owned(), Value::Text(input.object_uri)),
            (":graph_uri".to_owned(), Value::Text(input.graph_uri)),
        ];
        let board_sql = if let Some(board_id) = board_id {
            params.push((":board_id".to_owned(), Value::Text(board_id)));
            " AND board_id = :board_id"
        } else {
            ""
        };
        let affected = connection
            .execute(
                &format!("DELETE FROM entity_relations WHERE subject_uri = :subject_uri AND predicate = :predicate AND object_uri = :object_uri AND graph_uri = :graph_uri{board_sql}"),
                params,
            )
            .await?;
        Ok(affected > 0)
    }
}

fn normalize_limit(limit: usize) -> Result<usize, StoreError> {
    if limit == 0 || limit > MAX_RELATION_LIMIT {
        return Err(StoreError::InvalidInput(format!(
            "relation limit must be between 1 and {MAX_RELATION_LIMIT}"
        )));
    }
    Ok(limit)
}

fn validate_uri(uri: &str) -> Result<(), StoreError> {
    if !uri.trim().starts_with("kb://") || uri.trim().len() <= 5 {
        return Err(StoreError::InvalidInput(
            "entity URI must start with kb://".to_owned(),
        ));
    }
    Ok(())
}

fn validate_predicate_input(input: &RelationPredicateInput) -> Result<(), StoreError> {
    if input.name.trim().is_empty() || input.name.chars().any(char::is_whitespace) {
        return Err(StoreError::InvalidInput(
            "predicate name must be non-empty and contain no whitespace".to_owned(),
        ));
    }
    if input.cardinality.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "predicate cardinality is required".to_owned(),
        ));
    }
    if input.authoritative_store.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "predicate authoritative_store is required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_relation_input(input: &RelationUpsertInput) -> Result<(), StoreError> {
    validate_uri(&input.subject_uri)?;
    validate_uri(&input.object_uri)?;
    validate_uri(&input.graph_uri)?;
    if input.predicate.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "relation predicate is required".to_owned(),
        ));
    }
    if input.authoritative_store.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "relation authoritative_store is required".to_owned(),
        ));
    }
    if input.metadata_json.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "relation metadata_json is required".to_owned(),
        ));
    }
    Ok(())
}

fn optional_text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |value| Value::Text(value.to_owned()))
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

async fn entity_board_id(
    transaction: &turso::transaction::Transaction<'_>,
    uri: &str,
) -> Result<Option<String>, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT board_id FROM entities WHERE uri = :uri LIMIT 1",
                [(":uri", uri)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::EntityNotFound(uri.to_owned()),
        other => StoreError::Turso(other),
    })?;
    optional_text_value(row.get_value(0)?, "entities.board_id")
}

fn predicate_from_row(row: turso::Row) -> Result<RelationPredicateRecord, StoreError> {
    Ok(RelationPredicateRecord {
        name: text_value(row.get_value(0)?, "relation_predicates.name")?,
        domain_kind: optional_text_value(row.get_value(1)?, "relation_predicates.domain_kind")?,
        range_kind: optional_text_value(row.get_value(2)?, "relation_predicates.range_kind")?,
        cardinality: text_value(row.get_value(3)?, "relation_predicates.cardinality")?,
        authoritative_store: text_value(
            row.get_value(4)?,
            "relation_predicates.authoritative_store",
        )?,
        description: optional_text_value(row.get_value(5)?, "relation_predicates.description")?,
        created_at: integer_value(row.get_value(6)?, "relation_predicates.created_at")?,
    })
}

pub(crate) fn relation_from_row(row: turso::Row) -> Result<RelationRecord, StoreError> {
    Ok(RelationRecord {
        id: integer_value(row.get_value(0)?, "entity_relations.id")?,
        subject_uri: text_value(row.get_value(1)?, "entity_relations.subject_uri")?,
        predicate: text_value(row.get_value(2)?, "entity_relations.predicate")?,
        object_uri: text_value(row.get_value(3)?, "entity_relations.object_uri")?,
        graph_uri: text_value(row.get_value(4)?, "entity_relations.graph_uri")?,
        board_id: optional_text_value(row.get_value(5)?, "entity_relations.board_id")?,
        authoritative_store: text_value(row.get_value(6)?, "entity_relations.authoritative_store")?,
        source_table: optional_text_value(row.get_value(7)?, "entity_relations.source_table")?,
        source_id: optional_text_value(row.get_value(8)?, "entity_relations.source_id")?,
        source_event_id: optional_integer_value(
            row.get_value(9)?,
            "entity_relations.source_event_id",
        )?,
        metadata_json: text_value(row.get_value(10)?, "entity_relations.metadata_json")?,
        created_at: integer_value(row.get_value(11)?, "entity_relations.created_at")?,
        updated_at: integer_value(row.get_value(12)?, "entity_relations.updated_at")?,
    })
}
