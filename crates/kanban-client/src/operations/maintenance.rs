use kanban_protocol::{
    BackupReport, BackupResponse, CheckpointReport, CheckpointResponse, DoctorReport,
    DoctorResponse, ExportReport, ExportResponse, ImportReport, ImportResponse, LegacyImportReport,
    LegacyImportRequest, LegacyImportResponse, MaintenanceImportRequest, MaintenanceOwnerStatus,
    MaintenancePathRequest, MaintenanceRunReport, MaintenanceRunRequest, MaintenanceRunResponse,
    MaintenanceStatusReport, MaintenanceStatusResponse, ProjectionStoreStatus, QueueStats,
    StatsResponse, VacuumReport, VacuumResponse,
};

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub fn doctor(&self) -> Result<DoctorReport, ClientError> {
        Ok(self
            .get::<DoctorResponse>("/api/v1/maintenance/doctor")?
            .data)
    }
    pub fn checkpoint(&self) -> Result<CheckpointReport, ClientError> {
        Ok(self
            .post::<_, CheckpointResponse>(
                "/api/v1/maintenance/checkpoint",
                &serde_json::json!({}),
            )?
            .data)
    }
    pub fn stats(&self, board: &str) -> Result<QueueStats, ClientError> {
        Ok(self
            .get::<StatsResponse>(&format!(
                "/api/v1/stats?board={}",
                crate::transport::encode_path_segment(board)
            ))?
            .data)
    }
    pub fn backup(&self, path: impl Into<String>) -> Result<BackupReport, ClientError> {
        Ok(self
            .post::<_, BackupResponse>(
                "/api/v1/maintenance/backup",
                &MaintenancePathRequest { path: path.into() },
            )?
            .data)
    }
    pub fn export(&self, path: impl Into<String>) -> Result<ExportReport, ClientError> {
        Ok(self
            .post::<_, ExportResponse>(
                "/api/v1/maintenance/export",
                &MaintenancePathRequest { path: path.into() },
            )?
            .data)
    }
    pub fn import(
        &self,
        path: impl Into<String>,
        replace: bool,
    ) -> Result<ImportReport, ClientError> {
        Ok(self
            .post::<_, ImportResponse>(
                "/api/v1/maintenance/import",
                &MaintenanceImportRequest {
                    path: path.into(),
                    replace,
                },
            )?
            .data)
    }
    pub fn import_legacy_sqlite_v30(
        &self,
        path: impl Into<String>,
        canonical_attachment_root: Option<String>,
    ) -> Result<LegacyImportReport, ClientError> {
        Ok(self
            .post::<_, LegacyImportResponse>(
                "/api/v1/maintenance/import-v30",
                &LegacyImportRequest {
                    path: path.into(),
                    canonical_attachment_root,
                },
            )?
            .data)
    }
    pub fn vacuum(&self) -> Result<VacuumReport, ClientError> {
        Ok(self
            .post::<_, VacuumResponse>("/api/v1/maintenance/vacuum", &serde_json::json!({}))?
            .data)
    }
    pub fn maintenance_status(&self) -> Result<MaintenanceStatusReport, ClientError> {
        Ok(self
            .get::<MaintenanceStatusResponse>("/api/v1/maintenance/status")?
            .data)
    }
    pub fn maintenance_run(
        &self,
        owner: Option<String>,
        action: Option<String>,
    ) -> Result<MaintenanceRunReport, ClientError> {
        Ok(self
            .post::<_, MaintenanceRunResponse>(
                "/api/v1/maintenance/run",
                &MaintenanceRunRequest { owner, action },
            )?
            .data)
    }
    pub fn maintenance_rebuild(
        &self,
        owner: Option<String>,
    ) -> Result<MaintenanceRunReport, ClientError> {
        Ok(self
            .post::<_, MaintenanceRunResponse>(
                "/api/v1/maintenance/rebuild",
                &MaintenanceRunRequest {
                    owner,
                    action: None,
                },
            )?
            .data)
    }
    pub fn maintenance_cleanup(
        &self,
        owner: Option<String>,
    ) -> Result<MaintenanceRunReport, ClientError> {
        Ok(self
            .post::<_, MaintenanceRunResponse>(
                "/api/v1/maintenance/cleanup",
                &MaintenanceRunRequest {
                    owner,
                    action: None,
                },
            )?
            .data)
    }
}

#[allow(dead_code)]
fn _typed_surface_witness(_: MaintenanceOwnerStatus, _: ProjectionStoreStatus) {}
