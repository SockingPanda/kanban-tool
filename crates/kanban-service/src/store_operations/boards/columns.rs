use crate::{db::TursoStore, domain::BoardColumnRecord, error::StoreError, shared::*};

impl TursoStore {
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
}
