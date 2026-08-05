use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

impl TursoStore {
    pub async fn list_expired_claims(
        &self,
        board_selector: &str,
        now: i64,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        if now < 0 {
            return Err(StoreError::InvalidInput(
                "now must be non-negative".to_owned(),
            ));
        }
        let board_selector = board_selector.trim();
        if board_selector.is_empty() {
            return Err(StoreError::InvalidInput("board is required".to_owned()));
        }

        let connection = self.connection().await?;
        let board = first_row(
            connection
                .query(
                    "SELECT id, archived_at FROM boards WHERE id = :board OR slug = :board LIMIT 1",
                    [(":board", board_selector)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(board.get_value(0)?, "boards.id")?;
        if optional_integer_value(board.get_value(1)?, "boards.archived_at")?.is_some() {
            return Ok(Vec::new());
        }

        let mut rows = connection
                .query(
                    &format!(
                        "{TASK_SELECT} WHERE t.board_id = :board_id AND b.archived_at IS NULL AND t.archived_at IS NULL AND t.status = 'running' AND t.claim_expires_at <= :now ORDER BY t.claim_expires_at ASC, t.id ASC"
                    ),
                    vec![
                        (":board_id".to_owned(), Value::Text(board_id.to_owned())),
                        (":now".to_owned(), Value::Integer(now)),
                    ],
                )
                .await?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().await? {
            tasks.push(task_from_row(row)?);
        }
        Ok(tasks)
    }
}
