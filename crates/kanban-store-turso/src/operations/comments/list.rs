use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

impl TursoStore {
    pub async fn list_comments(&self, task_id: &str) -> Result<Vec<CommentRecord>, StoreError> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(StoreError::InvalidInput(
                "task id must start with t_".to_owned(),
            ));
        }
        let connection = self.connection().await?;
        let task = first_row(
            connection
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
        let board_id = text_value(task.get_value(0)?, "tasks.board_id")?;
        let mut rows = connection
                .query(
                    "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND task_id = :task_id ORDER BY created_at ASC, id ASC",
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                    ],
                )
                .await?;
        let mut comments = Vec::new();
        while let Some(row) = rows.next().await? {
            comments.push(comment_from_row(row)?);
        }
        Ok(comments)
    }
}
