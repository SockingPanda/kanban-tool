use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::BoardRecord, error::StoreError, shared::*};

/// 归档看板时需要写入的审计信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveBoardInput {
    pub actor: String,
    pub event_id: String,
    pub archived_at: i64,
}

impl TursoStore {
    /// 在同一个事务中校验运行中任务、写入归档时间并追加归档事件。
    pub async fn archive_board(
        &self,
        selector: &str,
        input: ArchiveBoardInput,
    ) -> Result<BoardRecord, StoreError> {
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(StoreError::InvalidInput("board is required".to_owned()));
        }
        let input = ArchiveBoardInput {
            actor: input.actor.trim().to_owned(),
            event_id: input.event_id.trim().to_owned(),
            archived_at: input.archived_at,
        };
        if input.actor.trim().is_empty() {
            return Err(StoreError::InvalidInput("actor is required".to_owned()));
        }
        if !input.event_id.starts_with("e_") || input.event_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "event id must start with e_".to_owned(),
            ));
        }

        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board = first_row(
            transaction
                .query(
                    "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards WHERE id = ?1 OR slug = ?1 LIMIT 1",
                    [selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
            other => StoreError::Turso(other),
        })?;
        let board = board_from_row(board)?;
        if board.archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "board is already archived".to_owned(),
            ));
        }

        let running = first_row(
            transaction
                .query(
                    "SELECT EXISTS (SELECT 1 FROM tasks WHERE board_id = ?1 AND status = 'running') OR EXISTS (SELECT 1 FROM task_runs WHERE board_id = ?1 AND status = 'running')",
                    [board.id.as_str()],
                )
                .await?,
        )
        .await?;
        if integer_value(running.get_value(0)?, "running board work")? != 0 {
            return Err(StoreError::InvalidTransition(
                "cannot archive board with running work".to_owned(),
            ));
        }

        let changed = transaction
            .execute(
                "UPDATE boards SET archived_at = ?1, updated_at = ?1 WHERE id = ?2 AND archived_at IS NULL",
                (input.archived_at, board.id.as_str()),
            )
            .await?;
        if changed == 0 {
            return Err(StoreError::InvalidTransition(
                "cannot archive board".to_owned(),
            ));
        }

        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, NULL, NULL, 'board.archived', ?3, '{}', ?4)",
                (
                    input.event_id.as_str(),
                    board.id.as_str(),
                    input.actor.as_str(),
                    input.archived_at,
                ),
            )
            .await?;

        let result = board_from_row(
            first_row(
                transaction
                    .query(
                        "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards WHERE id = ?1 LIMIT 1",
                        [board.id.as_str()],
                    )
                    .await?,
            )
            .await?,
        )?;
        transaction.commit().await?;
        Ok(result)
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
