use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::BoardRecord, error::StoreError, schema, shared::*};

/// 创建看板时由 application service 生成的规范化输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBoardInput {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub actor: String,
    pub event_id: String,
    pub created_at: i64,
}

impl TursoStore {
    /// 在同一个 Turso 事务中创建看板、默认列和 `board.created` 事件。
    pub async fn create_board(&self, input: CreateBoardInput) -> Result<BoardRecord, StoreError> {
        let input = CreateBoardInput {
            id: input.id.trim().to_owned(),
            slug: input.slug.trim().to_owned(),
            name: input.name.trim().to_owned(),
            description: input
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            actor: input.actor.trim().to_owned(),
            event_id: input.event_id.trim().to_owned(),
            created_at: input.created_at,
        };
        validate_create_board_input(&input)?;

        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let duplicate = first_row(
            transaction
                .query(
                    "SELECT id FROM boards WHERE slug = ?1 LIMIT 1",
                    [input.slug.as_str()],
                )
                .await?,
        )
        .await;
        if duplicate.is_ok() {
            return Err(StoreError::InvalidInput(format!(
                "board slug already exists: {}",
                input.slug
            )));
        }
        if let Err(error) = duplicate {
            if !matches!(error, turso::Error::QueryReturnedNoRows) {
                return Err(StoreError::Turso(error));
            }
        }

        transaction
            .execute(
                "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5, NULL)",
                (
                    input.id.as_str(),
                    input.slug.as_str(),
                    input.name.as_str(),
                    input.description.as_deref(),
                    input.created_at,
                ),
            )
            .await
            .map_err(map_board_create_constraint)?;

        for (status, title, position, hidden) in schema::DEFAULT_COLUMNS {
            let column_id = format!("col_{}_{}", input.id.trim_start_matches("b_"), status);
            transaction
                .execute(
                    "INSERT INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
                    (
                        column_id.as_str(),
                        input.id.as_str(),
                        status,
                        title,
                        position,
                        i64::from(hidden),
                        input.created_at,
                    ),
                )
                .await?;
        }

        let payload = format!(r#"{{"slug":"{}"}}"#, input.slug);
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, NULL, NULL, 'board.created', ?3, ?4, ?5)",
                (
                    input.event_id.as_str(),
                    input.id.as_str(),
                    input.actor.as_str(),
                    payload.as_str(),
                    input.created_at,
                ),
            )
            .await?;

        let board = board_from_row(
            first_row(
                transaction
                    .query(
                        "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards WHERE id = ?1 LIMIT 1",
                        [input.id.as_str()],
                    )
                    .await?,
            )
            .await?,
        )?;
        transaction.commit().await?;
        Ok(board)
    }
}

fn validate_create_board_input(input: &CreateBoardInput) -> Result<(), StoreError> {
    if !input.id.starts_with("b_") || input.id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "board id must start with b_".to_owned(),
        ));
    }
    validate_board_slug(&input.slug)?;
    if input.name.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "board name is required".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.starts_with("e_") || input.event_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event id must start with e_".to_owned(),
        ));
    }
    Ok(())
}

fn validate_board_slug(slug: &str) -> Result<(), StoreError> {
    let reserved = ["b_", "t_", "r_", "c_", "a_", "l_", "col_", "e_"];
    if slug.is_empty()
        || slug.len() > 64
        || reserved.iter().any(|prefix| slug.starts_with(prefix))
        || !slug
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !slug.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(StoreError::InvalidInput(format!(
            "invalid board slug: {slug}"
        )));
    }
    Ok(())
}

fn map_board_create_constraint(error: turso::Error) -> StoreError {
    match error {
        turso::Error::Constraint(message) if message.contains("boards.slug") => {
            StoreError::InvalidInput("board slug already exists".to_owned())
        }
        other => StoreError::Turso(other),
    }
}

fn board_from_row(row: turso::Row) -> Result<BoardRecord, StoreError> {
    Ok(BoardRecord {
        id: text_value(row.get_value(0)?, "boards.id")?,
        slug: text_value(row.get_value(1)?, "boards.slug")?,
        name: text_value(row.get_value(2)?, "boards.name")?,
        description: optional_text_value(row.get_value(3)?, "boards.description")?,
        created_at: integer_value(row.get_value(4)?, "boards.created_at")?,
        updated_at: integer_value(row.get_value(5)?, "boards.updated_at")?,
        archived_at: optional_integer_value(row.get_value(6)?, "boards.archived_at")?,
    })
}
