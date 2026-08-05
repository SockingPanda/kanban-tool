use crate::{db::TursoStore, domain::BoardRecord, error::StoreError, shared::*};

impl TursoStore {
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
}
