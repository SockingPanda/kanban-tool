use std::path::Path;

use turso::{Builder, Connection, Database, transaction::TransactionBehavior};

use crate::{
    error::StoreError,
    schema,
    shared::{first_row, now_ms, text_value},
};

#[derive(Clone)]
pub struct TursoStore {
    database: Database,
}

impl TursoStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_str().ok_or(StoreError::InvalidPath)?;
        if path.is_empty() {
            return Err(StoreError::InvalidPath);
        }
        let database = Builder::new_local(path).build().await?;
        Ok(Self { database })
    }

    pub async fn initialize(&self) -> Result<(), StoreError> {
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        transaction.execute_batch(schema::CANONICAL_SCHEMA).await?;
        transaction
                .execute(
                    "INSERT OR IGNORE INTO schema_migrations(version, name, checksum, applied_at) VALUES (?1, ?2, '', ?3)",
                    (schema::SCHEMA_VERSION, schema::SCHEMA_NAME, now_ms()),
                )
                .await?;

        transaction
                .execute(
                    "INSERT OR IGNORE INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES ('b_default', 'default', 'Default', NULL, ?1, ?1, NULL)",
                    [now_ms()],
                )
                .await?;
        let board_id = first_row(
            transaction
                .query("SELECT id FROM boards WHERE slug = 'default'", ())
                .await?,
        )
        .await?
        .get_value(0)
        .map_err(StoreError::from)
        .and_then(|value| text_value(value, "boards.id"))?;

        for (status, title, position, hidden) in schema::DEFAULT_COLUMNS {
            let id = format!("col_{}_{}", board_id.trim_start_matches("b_"), status);
            transaction
                    .execute(
                        "INSERT OR IGNORE INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
                        (id, board_id.as_str(), status, title, position, hidden, now_ms()),
                    )
                    .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub(crate) async fn connection(&self) -> Result<Connection, StoreError> {
        let connection = self.database.connect()?;
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        Ok(connection)
    }
}
