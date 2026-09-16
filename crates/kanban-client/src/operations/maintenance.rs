use kanban_protocol::{
    BackupReport, CheckpointReport, DoctorReport, ExportReport, ImportReport, LegacyImportReport,
    MaintenanceOwnerStatus, MaintenanceRunReport, MaintenanceStatusReport, ProjectionStoreStatus,
    QueueStats, VacuumReport,
};

use crate::transport::rpc;

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub async fn doctor(&self) -> Result<DoctorReport, ClientError> {
        let response: kanban_protocol::DoctorResponse =
            rpc!(self, doctor, DoctorRequest, (), (), ())?;
        Ok(response.data)
    }
    pub async fn checkpoint(&self) -> Result<CheckpointReport, ClientError> {
        let response: kanban_protocol::CheckpointResponse =
            rpc!(self, checkpoint, CheckpointRequest, (), (), ())?;
        Ok(response.data)
    }
    pub async fn stats(&self, board: &str) -> Result<QueueStats, ClientError> {
        let response: kanban_protocol::StatsResponse = rpc!(
            self,
            get_stats,
            GetStatsRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(response.data)
    }
    pub async fn backup(&self, path: impl Into<String>) -> Result<BackupReport, ClientError> {
        let response: kanban_protocol::BackupResponse = rpc!(
            self,
            maintenance_backup,
            MaintenanceBackupRequest,
            (),
            (),
            kanban_protocol::MaintenancePathRequest { path: path.into() }
        )?;
        Ok(response.data)
    }
    pub async fn export(&self, path: impl Into<String>) -> Result<ExportReport, ClientError> {
        let response: kanban_protocol::ExportResponse = rpc!(
            self,
            maintenance_export,
            MaintenanceExportRequest,
            (),
            (),
            kanban_protocol::MaintenancePathRequest { path: path.into() }
        )?;
        Ok(response.data)
    }
    pub async fn import(
        &self,
        path: impl Into<String>,
        replace: bool,
    ) -> Result<ImportReport, ClientError> {
        let response: kanban_protocol::ImportResponse = rpc!(
            self,
            maintenance_import,
            MaintenanceImportRequest,
            (),
            (),
            kanban_protocol::MaintenanceImportRequest {
                path: path.into(),
                replace
            }
        )?;
        Ok(response.data)
    }
    pub async fn import_legacy_sqlite_v30(
        &self,
        path: impl Into<String>,
        canonical_attachment_root: Option<String>,
    ) -> Result<LegacyImportReport, ClientError> {
        let response: kanban_protocol::LegacyImportResponse = rpc!(
            self,
            maintenance_import_v30,
            MaintenanceImportV30Request,
            (),
            (),
            kanban_protocol::LegacyImportRequest {
                path: path.into(),
                canonical_attachment_root
            }
        )?;
        Ok(response.data)
    }
    pub async fn vacuum(&self) -> Result<VacuumReport, ClientError> {
        let response: kanban_protocol::VacuumResponse = rpc!(
            self,
            maintenance_vacuum,
            MaintenanceVacuumRequest,
            (),
            (),
            ()
        )?;
        Ok(response.data)
    }
    pub async fn maintenance_status(&self) -> Result<MaintenanceStatusReport, ClientError> {
        let response: kanban_protocol::MaintenanceStatusResponse = rpc!(
            self,
            maintenance_status,
            MaintenanceStatusRequest,
            (),
            (),
            ()
        )?;
        Ok(response.data)
    }
    pub async fn maintenance_run(
        &self,
        owner: Option<String>,
        action: Option<String>,
    ) -> Result<MaintenanceRunReport, ClientError> {
        let response: kanban_protocol::MaintenanceRunResponse = rpc!(
            self,
            maintenance_run,
            MaintenanceRunRequest,
            (),
            (),
            kanban_protocol::MaintenanceRunRequest { owner, action }
        )?;
        Ok(response.data)
    }
    pub async fn maintenance_rebuild(
        &self,
        owner: Option<String>,
    ) -> Result<MaintenanceRunReport, ClientError> {
        let response: kanban_protocol::MaintenanceRunResponse = rpc!(
            self,
            maintenance_rebuild,
            MaintenanceRebuildRequest,
            (),
            (),
            kanban_protocol::MaintenanceRunRequest {
                owner,
                action: None
            }
        )?;
        Ok(response.data)
    }
    pub async fn maintenance_cleanup(
        &self,
        owner: Option<String>,
    ) -> Result<MaintenanceRunReport, ClientError> {
        let response: kanban_protocol::MaintenanceRunResponse = rpc!(
            self,
            maintenance_cleanup,
            MaintenanceCleanupRequest,
            (),
            (),
            kanban_protocol::MaintenanceRunRequest {
                owner,
                action: None
            }
        )?;
        Ok(response.data)
    }
}

#[allow(dead_code)]
fn _typed_surface_witness(_: MaintenanceOwnerStatus, _: ProjectionStoreStatus) {}
