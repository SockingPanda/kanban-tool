use turso::transaction::Transaction;

use crate::{db::TursoStore, domain::BoardRecord, error::StoreError, shared::*};

impl TursoStore {
    /// 读取当前有效看板；归档看板通过此入口视为不存在。
    pub async fn get_board(&self, selector: &str) -> Result<BoardRecord, StoreError> {
        let selector = selector.trim();
        if selector.is_empty() {
            return Err(StoreError::InvalidInput("看板不能为空".to_owned()));
        }
        let connection = self.connection().await?;
        board_from_row(
            first_row(
                connection
                    .query(
                        "SELECT id, slug, name, description, created_at, updated_at, archived_at FROM boards WHERE (id = ?1 OR slug = ?1) AND archived_at IS NULL LIMIT 1",
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

/// 在归档事务中读取看板的完整历史状态；公开查询仍只允许有效看板。
pub(super) async fn get_board_including_archived(
    transaction: &Transaction<'_>,
    selector: &str,
) -> Result<BoardRecord, StoreError> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(StoreError::InvalidInput("看板不能为空".to_owned()));
    }
    board_from_row(
        first_row(
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
        })?,
    )
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
