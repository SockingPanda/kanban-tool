//! Host-admin 诊断与维护的 service boundary。

use kanban_core::{Clock, KanbanError, Result};

use crate::{
    KanbanService, StoreBackupReport, StoreCheckpointReport, StoreDoctorReport, StoreExportReport,
    StoreImportReport, StoreMaintenanceRun, StoreMaintenanceStatus, StoreVacuumReport,
};

/// host-admin 维护操作直接落在 canonical Turso primitive 上。
///
/// 该入口固定使用 `KanbanService` 的 service-owned store，保留统一的输入校验与
/// `StoreError` 到 `KanbanError` 的映射；不再为每个调用创建 application store trait。
impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn doctor(&self) -> Result<StoreDoctorReport> {
        self.application
            .store
            .store
            .doctor()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn checkpoint(&self) -> Result<StoreCheckpointReport> {
        self.application
            .store
            .store
            .checkpoint()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn backup(&self, path: &str) -> Result<StoreBackupReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .backup(path)
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn export(&self, path: &str) -> Result<StoreExportReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .export(path)
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn import(&self, path: &str, replace: bool) -> Result<StoreImportReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .import(path, replace)
            .await
            .map_err(crate::adapter::store_error)
    }

    /// 仅执行 replace 的 prepare/verify 阶段，并保留 restart/publish 证据。
    pub async fn prepare_import(&self, path: &str) -> Result<StoreImportReport> {
        validate_path(path)?;
        self.application
            .store
            .store
            .prepare_import(path)
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn vacuum(&self) -> Result<StoreVacuumReport> {
        self.application
            .store
            .store
            .vacuum()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn maintenance_status(&self) -> Result<StoreMaintenanceStatus> {
        self.application
            .store
            .store
            .maintenance_status()
            .await
            .map_err(crate::adapter::store_error)
    }

    pub async fn maintenance_run(&self, owner: &str, action: &str) -> Result<StoreMaintenanceRun> {
        if owner.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "maintenance owner 不能为空".to_owned(),
            ));
        }
        let action = action.trim();
        if !matches!(action, "run" | "rebuild" | "cleanup" | "compact") {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported maintenance action: {action}"
            )));
        }
        self.application
            .store
            .store
            .maintenance_run(owner, action)
            .await
            .map_err(crate::adapter::store_error)
    }

    #[cfg(feature = "legacy-sqlite-import")]
    pub async fn import_legacy_sqlite_v30(
        &self,
        options: crate::LegacyImportOptions,
    ) -> Result<crate::LegacyImportResult> {
        let source_path = options.source_path.to_string_lossy();
        validate_path(&source_path)?;
        if let Some(root) = options.canonical_attachment_root.as_deref() {
            let root = root.to_string_lossy();
            validate_path(&root)?;
        }
        self.application
            .store
            .store
            .import_legacy_sqlite_v30(options)
            .await
            .map_err(crate::adapter::store_error)
    }
}

fn validate_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        Err(KanbanError::InvalidInput("path 不能为空".to_owned()))
    } else {
        Ok(())
    }
}
