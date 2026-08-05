use crate::{db::TursoStore, domain::BoardRecord, error::StoreError, shared::*};

impl TursoStore {
    /// 读取看板及其历史状态；归档看板仍可通过此入口查看。
    pub async fn get_board(&self, selector: &str) -> Result<BoardRecord, StoreError> {
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(StoreError::InvalidInput("board is required".to_owned()));
        }
        let connection = self.connection().await?;
        board_from_row(
            first_row(
                connection
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
            })?,
        )
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
