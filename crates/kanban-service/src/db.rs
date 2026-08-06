#![doc = include_str!("../docs/persistence.md")]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use turso::{Builder, Connection, Database, transaction::TransactionBehavior};

use crate::{
    error::StoreError,
    migration, schema,
    shared::{first_row, now_ms, text_value},
};

/// 升级前备份的请求。内置流程已生成并验证快照；调用方可在 hook 中记录、追加验证或拒绝升级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpgradeBackupRequest {
    pub source_path: PathBuf,
    pub backup_path: PathBuf,
    pub family: String,
    pub from_version: i64,
    pub to_version: i64,
    pub fingerprint: String,
}

/// 宿主可以实现这个 seam，追加自己的备份登记或策略校验。
pub(crate) trait UpgradeBackupHook: Send + Sync {
    fn before_upgrade(&self, request: &UpgradeBackupRequest) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapabilityRecord {
    pub capability: String,
    pub available: bool,
    pub detail: String,
    pub checked_at: i64,
}

#[derive(Clone)]
pub(crate) struct TursoStore {
    database: Database,
    path: Arc<PathBuf>,
}

impl TursoStore {
    pub(crate) async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_str().ok_or(StoreError::InvalidPath)?;
        if path.is_empty() {
            return Err(StoreError::InvalidPath);
        }
        let database = Builder::new_local(path)
            .experimental_index_method(true)
            .experimental_vacuum(true)
            .build()
            .await?;
        Ok(Self {
            database,
            path: Arc::new(PathBuf::from(path)),
        })
    }

    pub(crate) async fn initialize(&self) -> Result<(), StoreError> {
        self.initialize_with_backup_hook(None).await
    }

    /// 将旧 SQLite v30 逻辑导入当前 Turso canonical 数据库。
    #[cfg(feature = "legacy-sqlite-import")]
    pub(crate) async fn import_legacy_sqlite_v30(
        &self,
        options: crate::legacy_import::LegacyImportOptions,
    ) -> Result<crate::legacy_import::LegacyImportResult, StoreError> {
        crate::legacy_import::import_into_store(self, options).await
    }

    /// `import_legacy_sqlite_v30` 的简短别名，供 host 管理 operation 使用。
    #[cfg(feature = "legacy-sqlite-import")]
    pub(crate) async fn import_legacy_sqlite(
        &self,
        options: crate::legacy_import::LegacyImportOptions,
    ) -> Result<crate::legacy_import::LegacyImportResult, StoreError> {
        self.import_legacy_sqlite_v30(options).await
    }

    /// 使用可选的升级前备份 hook 初始化。没有升级时不会调用 hook。
    pub(crate) async fn initialize_with_backup_hook(
        &self,
        backup_hook: Option<&dyn UpgradeBackupHook>,
    ) -> Result<(), StoreError> {
        let mut connection = self.connection().await?;
        migration::apply(&mut connection, self.path.as_ref(), backup_hook).await?;
        connection
            .execute_batch(schema::PROJECTION_TRIGGER_SCHEMA)
            .await?;

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
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
        self.refresh_capabilities().await?;
        Ok(())
    }

    /// 在内置升级备份之外追加宿主 hook；hook 失败会阻止 schema 事务开始。
    pub(crate) async fn initialize_requiring_backup(
        &self,
        backup_hook: &dyn UpgradeBackupHook,
    ) -> Result<(), StoreError> {
        self.initialize_with_backup_hook(Some(backup_hook)).await
    }

    pub(crate) async fn capability_report(&self) -> Result<Vec<CapabilityRecord>, StoreError> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT capability, available, detail, checked_at FROM schema_capabilities ORDER BY capability",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            let capability = text_value(row.get_value(0)?, "schema_capabilities.capability")?;
            let available = match row.get_value(1)? {
                turso::Value::Integer(value) => value != 0,
                _ => {
                    return Err(StoreError::InvalidStoredValue {
                        field: "schema_capabilities.available",
                    });
                }
            };
            let detail = text_value(row.get_value(2)?, "schema_capabilities.detail")?;
            let checked_at = match row.get_value(3)? {
                turso::Value::Integer(value) => value,
                _ => {
                    return Err(StoreError::InvalidStoredValue {
                        field: "schema_capabilities.checked_at",
                    });
                }
            };
            result.push(CapabilityRecord {
                capability,
                available,
                detail,
                checked_at,
            });
        }
        Ok(result)
    }

    async fn refresh_capabilities(&self) -> Result<(), StoreError> {
        let mut connection = self.connection().await?;
        let now = now_ms();
        let vector = match connection
            .query(
                "SELECT vector_distance_cos(vector32('[1.0, 0.0]'), vector32('[1.0, 0.0]'))",
                (),
            )
            .await
        {
            Ok(mut rows) => {
                while rows.next().await?.is_some() {}
                (true, "Turso vector32/vector_distance_cos 可用".to_owned())
            }
            Err(error) => (false, format!("vector32 capability 不可用: {error}")),
        };
        // v2 曾使用过旧索引名；它是可重建的 derived object，启动时先清掉旧名，
        // 再保证唯一的 task_search_fts provider index。
        let fts = match connection
            .execute("DROP INDEX IF EXISTS idx_retrieval_documents_fts", ())
            .await
        {
            Ok(_) => match connection.execute(schema::FTS_SCHEMA, ()).await {
                Ok(_) => (true, "Turso FTS index 可用".to_owned()),
                Err(error) => (
                    false,
                    format!("Turso FTS 不可用；请确认启用了 turso `fts` feature: {error}"),
                ),
            },
            Err(error) => (false, format!("Turso FTS 旧索引清理失败: {error}")),
        };
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        for (capability, (available, detail)) in [("vector32", vector), ("fts", fts)] {
            transaction
                .execute(
                    "INSERT INTO schema_capabilities(capability, available, detail, checked_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(capability) DO UPDATE SET available=excluded.available, detail=excluded.detail, checked_at=excluded.checked_at",
                    (capability, i64::from(available), detail.as_str(), now),
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

    pub(crate) fn database_path(&self) -> &Path {
        self.path.as_path()
    }
}
