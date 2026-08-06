use crate::label_surfaces::*;
use crate::{cli_helpers::*, cli_labels::*, cli_operator::*};

use std::collections::BTreeMap;

use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Map, Value};

use crate::{
    AddDependencyPath, AddDependencyRequest, AddDependencyResponse, AddTaskLabelPath,
    AddTaskLabelRequest, AddTaskLabelResponse, ArchiveTaskPath, ArchiveTaskRequest,
    ArchiveTaskResponse, AttachmentDownloadResponse, BlockTaskPath, BlockTaskRequest,
    BlockTaskResponse, BoardTaskMapPath, BoardTaskMapQuery, BoardTaskMapResponse,
    CheckpointResponse, ClaimTaskPath, ClaimTaskRequest, ClaimTaskResponse, CliAttachmentAddOutput,
    CliAttachmentListOutput, CliAttachmentRemoveOutput, CliBoardCurrentOutput, CliBoardUseOutput,
    CliCheckpointOutput, CliCommentAddOutput, CliCommentListOutput, CliConfigShowOutput,
    CliDependencyAddOutput, CliDependencyListOutput, CliDependencyRemoveOutput, CliDoctorOutput,
    CliEntityListOutput, CliEntityShowOutput, CliEntityUpsertOutput, CliEventsOutput,
    CliIndexDoctorOutput, CliIndexStatusOutput, CliInitOutput, CliRunLogsOutput, CliRunShowOutput,
    CliRunsOutput, CliStatsOutput, CliTaskArchiveOutput, CliTaskBlockOutput, CliTaskClaimOutput,
    CliTaskCreateOutput, CliTaskDoneOutput, CliTaskHeartbeatOutput, CliTaskListOutput,
    CliTaskPromoteOutput, CliTaskReclaimOutput, CliTaskReleaseOutput, CliTaskReopenOutput,
    CliTaskReviewOutput, CliTaskShowOutput, CliTaskSpecifyOutput, CliTaskStepAddOutput,
    CliTaskStepDoneOutput, CliTaskStepListOutput, CliTaskStepNotRequiredOutput,
    CliTaskStepRemoveOutput, CliTaskStepReopenOutput, CliTaskStepSkipOutput,
    CliTaskStepUpdateOutput, CliTaskUnblockOutput, CliTaskUpdateOutput, CompleteStepPath,
    CompleteStepRequest, CompleteStepResponse, CompleteTaskPath, CompleteTaskRequest,
    CompleteTaskResponse, ConfirmSignalsResponse, ContractDirection, ContractStrictness,
    CreateAttachmentPath, CreateAttachmentRequest, CreateAttachmentResponse, CreateCommentPath,
    CreateCommentRequest, CreateCommentResponse, CreateStepPath, CreateStepRequest,
    CreateStepResponse, CreateTaskPath, CreateTaskRequest, CreateTaskResponse, DecisionMetadata,
    DeleteAttachmentPath, DeleteAttachmentResponse, DeleteResponse, DoctorResponse, ErrorEnvelope,
    GetRunLogPath, GetRunLogResponse, GetRunPath, GetRunResponse, GetSignalResponse, GetTaskPath,
    GetTaskQuery, GetTaskResponse, HealthResponse, HeartbeatTaskPath, HeartbeatTaskRequest,
    HeartbeatTaskResponse, LabelOntologySignalsResponse, ListAttachmentsPath,
    ListAttachmentsResponse, ListCommentsPath, ListCommentsResponse, ListDependenciesPath,
    ListDependenciesResponse, ListEventsResponse, ListRunsPath, ListRunsResponse,
    ListSignalsResponse, ListStepsPath, ListStepsResponse, ListTaskLabelsPath,
    ListTaskLabelsResponse, ListTasksByStatusPath, ListTasksByStatusQuery,
    ListTasksByStatusResponse, ListTasksPath, ListTasksQuery, ListTasksResponse,
    MarkExecutionPlanNotRequiredPath, MarkExecutionPlanNotRequiredRequest,
    MarkExecutionPlanNotRequiredResponse, PromoteTaskPath, PromoteTaskRequest, PromoteTaskResponse,
    ReclaimTaskPath, ReclaimTaskRequest, ReclaimTaskResponse, RecordSignalRequest,
    RecordSignalResponse, RejectSignalsResponse, ReleaseTaskPath, ReleaseTaskRequest,
    ReleaseTaskResponse, RemoveDependencyPath, RemoveDependencyResponse, RemoveStepPath,
    RemoveStepResponse, RemoveTaskLabelPath, RemoveTaskLabelResponse, ReopenStepPath,
    ReopenStepRequest, ReopenStepResponse, ReopenTaskPath, ReopenTaskRequest, ReopenTaskResponse,
    ResolveSignalsResponse, ReviewSignalsRequest, ReviewSignalsResponse, SkipStepPath,
    SkipStepRequest, SkipStepResponse, SpecifyTaskPath, SpecifyTaskRequest, SpecifyTaskResponse,
    StreamEventData, StreamEventsQuery, SubmitReviewTaskPath, SubmitReviewTaskRequest,
    SubmitReviewTaskResponse, SupersedeSignalsResponse, TaskNeighborhoodPath,
    TaskNeighborhoodQuery, TaskNeighborhoodResponse, UnblockTaskPath, UnblockTaskRequest,
    UnblockTaskResponse, UpdateStepPath, UpdateStepRequest, UpdateStepResponse, UpdateTaskPath,
    UpdateTaskRequest, UpdateTaskResponse,
};

use crate::{
    BackupResponse, BoardQuery, BuildContextPath, BuildContextQuery, BuildContextResponse,
    EntityListQuery, EntityListResponse, EntityPath, EntityResponse, EntityUpsertRequest,
    ExportResponse, GraphMaintenanceResponse, GraphNeighborsQuery, GraphNeighborsResponse,
    GraphQueryQuery, GraphStatusResponse, ImportResponse, LegacyImportRequest,
    LegacyImportResponse, ListEventsQuery, MaintenanceImportRequest, MaintenancePathRequest,
    MaintenanceRunRequest, MaintenanceRunResponse, MaintenanceStatusResponse, SearchStatusResponse,
    SearchTasksByStatusResponse, SearchTasksQuery, SearchTasksResponse, StatsResponse,
    VacuumResponse, VectorConfigureRequest, VectorConfigureResponse, VectorProjectionRequest,
    VectorProjectionResponse, VectorQuery, VectorQueryChunksResponse,
    VectorQueryLabelAtomsResponse, VectorStatusQuery, VectorStatusResponse,
};

pub const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

#[derive(Debug, Clone, Copy)]
pub struct SchemaRoot {
    pub id: &'static str,
    pub artifact_path: &'static str,
    pub title: &'static str,
    pub contract_id: &'static str,
    pub direction: ContractDirection,
    pub strictness: ContractStrictness,
    pub valid_fixture: &'static str,
    pub invalid_fixture: &'static str,
    pub(crate) generate: fn(ContractDirection) -> Value,
}

macro_rules! request_schema_root {
    (
        $id:literal,
        $artifact:literal,
        $title:literal,
        $contract_id:literal,
        $valid_fixture:literal,
        $invalid_fixture:literal,
        $request:ty
    ) => {
        SchemaRoot {
            id: $id,
            artifact_path: $artifact,
            title: $title,
            contract_id: $contract_id,
            direction: ContractDirection::Deserialize,
            strictness: ContractStrictness::DenyUnknownFields,
            valid_fixture: $valid_fixture,
            invalid_fixture: $invalid_fixture,
            generate: generate_for::<$request>,
        }
    };
}

macro_rules! response_schema_root {
    ($id:literal, $artifact:literal, $title:literal, $contract_id:literal, $valid:literal, $invalid:literal, $response:ty) => {
        SchemaRoot {
            id: $id,
            artifact_path: $artifact,
            title: $title,
            contract_id: $contract_id,
            direction: ContractDirection::Serialize,
            strictness: ContractStrictness::DenyUnknownFields,
            valid_fixture: $valid,
            invalid_fixture: $invalid,
            generate: generate_for::<$response>,
        }
    };
}

macro_rules! cli_response_schema_root {
    ($slug:literal, $operation:literal, $response:ty) => {
        SchemaRoot {
            id: concat!("urn:kanban-tool:schema:cli:", $slug, "-output:v1"),
            artifact_path: concat!("cli/", $slug, "-output.v1.schema.json"),
            title: concat!("Kanban CLI ", $operation, " output v1"),
            contract_id: concat!("cli.", $slug, ".output"),
            direction: ContractDirection::Serialize,
            strictness: ContractStrictness::DenyUnknownFields,
            valid_fixture: concat!("schemas/fixtures/cli/", $slug, "-output.v1.valid.json"),
            invalid_fixture: concat!("schemas/fixtures/cli/", $slug, "-output.v1.invalid.json"),
            generate: generate_for::<$response>,
        }
    };
}

const SCHEMA_REGISTRY: &[SchemaRoot] = &[
    response_schema_root!(
        "urn:kanban-tool:schema:cli:init-output:v1",
        "cli/init-output.v1.schema.json",
        "Kanban CLI init output v1",
        "cli.init.output",
        "schemas/fixtures/cli/init-output.v1.valid.json",
        "schemas/fixtures/cli/init-output.v1.invalid.json",
        CliInitOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:config-show-output:v1",
        "cli/config-show-output.v1.schema.json",
        "Kanban CLI config show output v1",
        "cli.config-show.output",
        "schemas/fixtures/cli/config-show-output.v1.valid.json",
        "schemas/fixtures/cli/config-show-output.v1.invalid.json",
        CliConfigShowOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:index-status-output:v1",
        "cli/index-status-output.v1.schema.json",
        "Kanban CLI index status output v1",
        "cli.index-status.output",
        "schemas/fixtures/cli/index-status-output.v1.valid.json",
        "schemas/fixtures/cli/index-status-output.v1.invalid.json",
        CliIndexStatusOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:index-doctor-output:v1",
        "cli/index-doctor-output.v1.schema.json",
        "Kanban CLI index doctor output v1",
        "cli.index-doctor.output",
        "schemas/fixtures/cli/index-doctor-output.v1.valid.json",
        "schemas/fixtures/cli/index-doctor-output.v1.invalid.json",
        CliIndexDoctorOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:entity-list-output:v1",
        "cli/entity-list-output.v1.schema.json",
        "Kanban CLI entity list output v1",
        "cli.entity-list.output",
        "schemas/fixtures/cli/entity-list-output.v1.valid.json",
        "schemas/fixtures/cli/entity-list-output.v1.invalid.json",
        CliEntityListOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:entity-show-output:v1",
        "cli/entity-show-output.v1.schema.json",
        "Kanban CLI entity show output v1",
        "cli.entity-show.output",
        "schemas/fixtures/cli/entity-show-output.v1.valid.json",
        "schemas/fixtures/cli/entity-show-output.v1.invalid.json",
        CliEntityShowOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:entity-upsert-output:v1",
        "cli/entity-upsert-output.v1.schema.json",
        "Kanban CLI entity upsert output v1",
        "cli.entity-upsert.output",
        "schemas/fixtures/cli/entity-upsert-output.v1.valid.json",
        "schemas/fixtures/cli/entity-upsert-output.v1.invalid.json",
        CliEntityUpsertOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:doctor-output:v1",
        "cli/doctor-output.v1.schema.json",
        "Kanban CLI doctor output v1",
        "cli.doctor.output",
        "schemas/fixtures/cli/doctor-output.v1.valid.json",
        "schemas/fixtures/cli/doctor-output.v1.invalid.json",
        CliDoctorOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:stats-output:v1",
        "cli/stats-output.v1.schema.json",
        "Kanban CLI stats output v1",
        "cli.stats.output",
        "schemas/fixtures/cli/stats-output.v1.valid.json",
        "schemas/fixtures/cli/stats-output.v1.invalid.json",
        CliStatsOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:checkpoint-output:v1",
        "cli/checkpoint-output.v1.schema.json",
        "Kanban CLI checkpoint output v1",
        "cli.checkpoint.output",
        "schemas/fixtures/cli/checkpoint-output.v1.valid.json",
        "schemas/fixtures/cli/checkpoint-output.v1.invalid.json",
        CliCheckpointOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-backup-output:v1",
        "cli/maintenance-backup-output.v1.schema.json",
        "Kanban CLI maintenance backup output v1",
        "cli.maintenance-backup.output",
        "schemas/fixtures/cli/maintenance-backup-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-backup-output.v1.invalid.json",
        BackupResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-export-output:v1",
        "cli/maintenance-export-output.v1.schema.json",
        "Kanban CLI maintenance export output v1",
        "cli.maintenance-export.output",
        "schemas/fixtures/cli/maintenance-export-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-export-output.v1.invalid.json",
        ExportResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-import-output:v1",
        "cli/maintenance-import-output.v1.schema.json",
        "Kanban CLI maintenance import output v1",
        "cli.maintenance-import.output",
        "schemas/fixtures/cli/maintenance-import-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-import-output.v1.invalid.json",
        ImportResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-vacuum-output:v1",
        "cli/maintenance-vacuum-output.v1.schema.json",
        "Kanban CLI maintenance vacuum output v1",
        "cli.maintenance-vacuum.output",
        "schemas/fixtures/cli/maintenance-vacuum-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-vacuum-output.v1.invalid.json",
        VacuumResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-status-output:v1",
        "cli/maintenance-status-output.v1.schema.json",
        "Kanban CLI maintenance status output v1",
        "cli.maintenance-status-v1.output",
        "schemas/fixtures/cli/maintenance-status-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-status-output.v1.invalid.json",
        MaintenanceStatusResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-run-output:v1",
        "cli/maintenance-run-output.v1.schema.json",
        "Kanban CLI maintenance run output v1",
        "cli.maintenance-run-v1.output",
        "schemas/fixtures/cli/maintenance-run-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-run-output.v1.invalid.json",
        MaintenanceRunResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-rebuild-output.v1",
        "cli/maintenance-rebuild-output.v1.schema.json",
        "Kanban CLI maintenance rebuild output v1",
        "cli.maintenance-rebuild-v1.output",
        "schemas/fixtures/cli/maintenance-rebuild-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-rebuild-output.v1.invalid.json",
        MaintenanceRunResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:maintenance-cleanup-output.v1",
        "cli/maintenance-cleanup-output.v1.schema.json",
        "Kanban CLI maintenance cleanup output v1",
        "cli.maintenance-cleanup.output",
        "schemas/fixtures/cli/maintenance-cleanup-output.v1.valid.json",
        "schemas/fixtures/cli/maintenance-cleanup-output.v1.invalid.json",
        MaintenanceRunResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:import-v30-output:v1",
        "cli/import-v30-output.v1.schema.json",
        "Kanban CLI legacy SQLite v30 import output v1",
        "cli.import-v30.output",
        "schemas/fixtures/cli/import-v30-output.v1.valid.json",
        "schemas/fixtures/cli/import-v30-output.v1.invalid.json",
        LegacyImportResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:board-use-output:v1",
        "cli/board-use-output.v1.schema.json",
        "Kanban CLI board use output v1",
        "cli.board-use.output",
        "schemas/fixtures/cli/board-use-output.v1.valid.json",
        "schemas/fixtures/cli/board-use-output.v1.invalid.json",
        CliBoardUseOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:board-current-output:v1",
        "cli/board-current-output.v1.schema.json",
        "Kanban CLI board current output v1",
        "cli.board-current.output",
        "schemas/fixtures/cli/board-current-output.v1.valid.json",
        "schemas/fixtures/cli/board-current-output.v1.invalid.json",
        CliBoardCurrentOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-list-output:v1",
        "cli/task-list-output.v1.schema.json",
        "Kanban CLI task list output v1",
        "cli.task-list.output",
        "schemas/fixtures/cli/task-list-output.v1.valid.json",
        "schemas/fixtures/cli/task-list-output.v1.invalid.json",
        CliTaskListOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-show-output:v1",
        "cli/task-show-output.v1.schema.json",
        "Kanban CLI task show output v1",
        "cli.task-show.output",
        "schemas/fixtures/cli/task-show-output.v1.valid.json",
        "schemas/fixtures/cli/task-show-output.v1.invalid.json",
        CliTaskShowOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-specify-output:v1",
        "cli/task-specify-output.v1.schema.json",
        "Kanban CLI task specify output v1",
        "cli.task-specify.output",
        "schemas/fixtures/cli/task-specify-output.v1.valid.json",
        "schemas/fixtures/cli/task-specify-output.v1.invalid.json",
        CliTaskSpecifyOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:comment-add-output:v1",
        "cli/comment-add-output.v1.schema.json",
        "Kanban CLI comment add output v1",
        "cli.comment-add.output",
        "schemas/fixtures/cli/comment-add-output.v1.valid.json",
        "schemas/fixtures/cli/comment-add-output.v1.invalid.json",
        CliCommentAddOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:comment-list-output:v1",
        "cli/comment-list-output.v1.schema.json",
        "Kanban CLI comment list output v1",
        "cli.comment-list.output",
        "schemas/fixtures/cli/comment-list-output.v1.valid.json",
        "schemas/fixtures/cli/comment-list-output.v1.invalid.json",
        CliCommentListOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:dep-add-output:v1",
        "cli/dep-add-output.v1.schema.json",
        "Kanban CLI dependency add output v1",
        "cli.dep-add.output",
        "schemas/fixtures/cli/dep-add-output.v1.valid.json",
        "schemas/fixtures/cli/dep-add-output.v1.invalid.json",
        CliDependencyAddOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:dep-list-output:v1",
        "cli/dep-list-output.v1.schema.json",
        "Kanban CLI dependency list output v1",
        "cli.dep-list.output",
        "schemas/fixtures/cli/dep-list-output.v1.valid.json",
        "schemas/fixtures/cli/dep-list-output.v1.invalid.json",
        CliDependencyListOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:dep-remove-output:v1",
        "cli/dep-remove-output.v1.schema.json",
        "Kanban CLI dependency remove output v1",
        "cli.dep-remove.output",
        "schemas/fixtures/cli/dep-remove-output.v1.valid.json",
        "schemas/fixtures/cli/dep-remove-output.v1.invalid.json",
        CliDependencyRemoveOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:events-output:v1",
        "cli/events-output.v1.schema.json",
        "Kanban CLI events output v1",
        "cli.events.output",
        "schemas/fixtures/cli/events-output.v1.valid.json",
        "schemas/fixtures/cli/events-output.v1.invalid.json",
        CliEventsOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-list-output:v1",
        "cli/task-step-list-output.v1.schema.json",
        "Kanban CLI task step list output v1",
        "cli.task-step-list.output",
        "schemas/fixtures/cli/task-step-list-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-list-output.v1.invalid.json",
        CliTaskStepListOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-add-output:v1",
        "cli/task-step-add-output.v1.schema.json",
        "Kanban CLI task step add output v1",
        "cli.task-step-add.output",
        "schemas/fixtures/cli/task-step-add-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-add-output.v1.invalid.json",
        CliTaskStepAddOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-update-output:v1",
        "cli/task-step-update-output.v1.schema.json",
        "Kanban CLI task step update output v1",
        "cli.task-step-update.output",
        "schemas/fixtures/cli/task-step-update-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-update-output.v1.invalid.json",
        CliTaskStepUpdateOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-done-output:v1",
        "cli/task-step-done-output.v1.schema.json",
        "Kanban CLI task step done output v1",
        "cli.task-step-done.output",
        "schemas/fixtures/cli/task-step-done-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-done-output.v1.invalid.json",
        CliTaskStepDoneOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-skip-output:v1",
        "cli/task-step-skip-output.v1.schema.json",
        "Kanban CLI task step skip output v1",
        "cli.task-step-skip.output",
        "schemas/fixtures/cli/task-step-skip-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-skip-output.v1.invalid.json",
        CliTaskStepSkipOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-reopen-output:v1",
        "cli/task-step-reopen-output.v1.schema.json",
        "Kanban CLI task step reopen output v1",
        "cli.task-step-reopen.output",
        "schemas/fixtures/cli/task-step-reopen-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-reopen-output.v1.invalid.json",
        CliTaskStepReopenOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-remove-output:v1",
        "cli/task-step-remove-output.v1.schema.json",
        "Kanban CLI task step remove output v1",
        "cli.task-step-remove.output",
        "schemas/fixtures/cli/task-step-remove-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-remove-output.v1.invalid.json",
        CliTaskStepRemoveOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-step-not-required-output:v1",
        "cli/task-step-not-required-output.v1.schema.json",
        "Kanban CLI task step not-required output v1",
        "cli.task-step-not-required.output",
        "schemas/fixtures/cli/task-step-not-required-output.v1.valid.json",
        "schemas/fixtures/cli/task-step-not-required-output.v1.invalid.json",
        CliTaskStepNotRequiredOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:runs-output:v1",
        "cli/runs-output.v1.schema.json",
        "Kanban CLI runs output v1",
        "cli.runs.output",
        "schemas/fixtures/cli/runs-output.v1.valid.json",
        "schemas/fixtures/cli/runs-output.v1.invalid.json",
        CliRunsOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:run-show-output:v1",
        "cli/run-show-output.v1.schema.json",
        "Kanban CLI run show output v1",
        "cli.run-show.output",
        "schemas/fixtures/cli/run-show-output.v1.valid.json",
        "schemas/fixtures/cli/run-show-output.v1.invalid.json",
        CliRunShowOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:run-logs-output:v1",
        "cli/run-logs-output.v1.schema.json",
        "Kanban CLI run logs output v1",
        "cli.run-logs.output",
        "schemas/fixtures/cli/run-logs-output.v1.valid.json",
        "schemas/fixtures/cli/run-logs-output.v1.invalid.json",
        CliRunLogsOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-create-output:v1",
        "cli/task-create-output.v1.schema.json",
        "Kanban CLI task create output v1",
        "cli.task-create.output",
        "schemas/fixtures/cli/task-create-output.v1.valid.json",
        "schemas/fixtures/cli/task-create-output.v1.invalid.json",
        CliTaskCreateOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-update-output:v1",
        "cli/task-update-output.v1.schema.json",
        "Kanban CLI task update output v1",
        "cli.task-update.output",
        "schemas/fixtures/cli/task-update-output.v1.valid.json",
        "schemas/fixtures/cli/task-update-output.v1.invalid.json",
        CliTaskUpdateOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-claim-output:v1",
        "cli/task-claim-output.v1.schema.json",
        "Kanban CLI task claim output v1",
        "cli.task-claim.output",
        "schemas/fixtures/cli/task-claim-output.v1.valid.json",
        "schemas/fixtures/cli/task-claim-output.v1.invalid.json",
        CliTaskClaimOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-reclaim-output:v1",
        "cli/task-reclaim-output.v1.schema.json",
        "Kanban CLI task reclaim output v1",
        "cli.task-reclaim.output",
        "schemas/fixtures/cli/task-reclaim-output.v1.valid.json",
        "schemas/fixtures/cli/task-reclaim-output.v1.invalid.json",
        CliTaskReclaimOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-promote-output:v1",
        "cli/task-promote-output.v1.schema.json",
        "Kanban CLI task promote output v1",
        "cli.task-promote.output",
        "schemas/fixtures/cli/task-promote-output.v1.valid.json",
        "schemas/fixtures/cli/task-promote-output.v1.invalid.json",
        CliTaskPromoteOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-reopen-output:v1",
        "cli/task-reopen-output.v1.schema.json",
        "Kanban CLI task reopen output v1",
        "cli.task-reopen.output",
        "schemas/fixtures/cli/task-reopen-output.v1.valid.json",
        "schemas/fixtures/cli/task-reopen-output.v1.invalid.json",
        CliTaskReopenOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-heartbeat-output:v1",
        "cli/task-heartbeat-output.v1.schema.json",
        "Kanban CLI task heartbeat output v1",
        "cli.task-heartbeat.output",
        "schemas/fixtures/cli/task-heartbeat-output.v1.valid.json",
        "schemas/fixtures/cli/task-heartbeat-output.v1.invalid.json",
        CliTaskHeartbeatOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-release-output:v1",
        "cli/task-release-output.v1.schema.json",
        "Kanban CLI task release output v1",
        "cli.task-release.output",
        "schemas/fixtures/cli/task-release-output.v1.valid.json",
        "schemas/fixtures/cli/task-release-output.v1.invalid.json",
        CliTaskReleaseOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-done-output:v1",
        "cli/task-done-output.v1.schema.json",
        "Kanban CLI task done output v1",
        "cli.task-done.output",
        "schemas/fixtures/cli/task-done-output.v1.valid.json",
        "schemas/fixtures/cli/task-done-output.v1.invalid.json",
        CliTaskDoneOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-review-output:v1",
        "cli/task-review-output.v1.schema.json",
        "Kanban CLI task review output v1",
        "cli.task-review.output",
        "schemas/fixtures/cli/task-review-output.v1.valid.json",
        "schemas/fixtures/cli/task-review-output.v1.invalid.json",
        CliTaskReviewOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-block-output:v1",
        "cli/task-block-output.v1.schema.json",
        "Kanban CLI task block output v1",
        "cli.task-block.output",
        "schemas/fixtures/cli/task-block-output.v1.valid.json",
        "schemas/fixtures/cli/task-block-output.v1.invalid.json",
        CliTaskBlockOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-unblock-output:v1",
        "cli/task-unblock-output.v1.schema.json",
        "Kanban CLI task unblock output v1",
        "cli.task-unblock.output",
        "schemas/fixtures/cli/task-unblock-output.v1.valid.json",
        "schemas/fixtures/cli/task-unblock-output.v1.invalid.json",
        CliTaskUnblockOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:cli:task-archive-output:v1",
        "cli/task-archive-output.v1.schema.json",
        "Kanban CLI task archive output v1",
        "cli.task-archive.output",
        "schemas/fixtures/cli/task-archive-output.v1.valid.json",
        "schemas/fixtures/cli/task-archive-output.v1.invalid.json",
        CliTaskArchiveOutput
    ),
    cli_response_schema_root!("label-add", "label add", CliLabelAddOutput),
    cli_response_schema_root!(
        "label-atom-index-query",
        "label atom-index query",
        CliLabelAtomIndexQueryOutput
    ),
    cli_response_schema_root!(
        "label-atom-index-rebuild",
        "label atom-index rebuild",
        CliLabelAtomIndexRebuildOutput
    ),
    cli_response_schema_root!(
        "label-atom-index-status",
        "label atom-index status",
        CliLabelAtomIndexStatusOutput
    ),
    cli_response_schema_root!(
        "label-atoms-explain",
        "label atoms explain",
        CliLabelAtomsExplainOutput
    ),
    cli_response_schema_root!(
        "label-atoms-list",
        "label atoms list",
        CliLabelAtomsListOutput
    ),
    cli_response_schema_root!("label-create", "label create", CliLabelCreateOutput),
    cli_response_schema_root!("label-list", "label list", CliLabelListOutput),
    cli_response_schema_root!(
        "label-ontology-apply-atom",
        "label ontology apply atom",
        CliLabelOntologyApplyAtomOutput
    ),
    cli_response_schema_root!(
        "label-ontology-confirm",
        "label ontology confirm",
        CliLabelOntologyConfirmOutput
    ),
    cli_response_schema_root!(
        "label-ontology-list",
        "label ontology list",
        CliLabelOntologyListOutput
    ),
    cli_response_schema_root!(
        "label-ontology-quality",
        "label ontology quality",
        CliLabelOntologyQualityOutput
    ),
    cli_response_schema_root!(
        "label-ontology-record",
        "label ontology record",
        CliLabelOntologyRecordOutput
    ),
    cli_response_schema_root!(
        "label-ontology-reject",
        "label ontology reject",
        CliLabelOntologyRejectOutput
    ),
    cli_response_schema_root!(
        "label-ontology-resolve",
        "label ontology resolve",
        CliLabelOntologyResolveOutput
    ),
    cli_response_schema_root!(
        "label-ontology-revert",
        "label ontology revert",
        CliLabelOntologyRevertOutput
    ),
    cli_response_schema_root!(
        "label-ontology-review",
        "label ontology review",
        CliLabelOntologyReviewOutput
    ),
    cli_response_schema_root!(
        "label-ontology-show",
        "label ontology show",
        CliLabelOntologyShowOutput
    ),
    cli_response_schema_root!(
        "label-ontology-supersede",
        "label ontology supersede",
        CliLabelOntologySupersedeOutput
    ),
    cli_response_schema_root!(
        "label-ontology-validate",
        "label ontology validate",
        CliLabelOntologyValidateOutput
    ),
    cli_response_schema_root!(
        "label-proposals-accept",
        "label proposals accept",
        CliLabelProposalsAcceptOutput
    ),
    cli_response_schema_root!(
        "label-proposals-list",
        "label proposals list",
        CliLabelProposalsListOutput
    ),
    cli_response_schema_root!(
        "label-proposals-reject",
        "label proposals reject",
        CliLabelProposalsRejectOutput
    ),
    cli_response_schema_root!(
        "label-proposals-show",
        "label proposals show",
        CliLabelProposalsShowOutput
    ),
    cli_response_schema_root!("label-propose", "label propose", CliLabelProposeOutput),
    cli_response_schema_root!("label-remove", "label remove", CliLabelRemoveOutput),
    cli_response_schema_root!(
        "label-semantics-delete",
        "label semantics delete",
        CliLabelSemanticsDeleteOutput
    ),
    cli_response_schema_root!(
        "label-semantics-list",
        "label semantics list",
        CliLabelSemanticsListOutput
    ),
    cli_response_schema_root!(
        "label-semantics-show",
        "label semantics show",
        CliLabelSemanticsShowOutput
    ),
    cli_response_schema_root!(
        "label-semantics-upsert",
        "label semantics upsert",
        CliLabelSemanticsUpsertOutput
    ),
    cli_response_schema_root!("label-suggest", "label suggest", CliLabelSuggestOutput),
    cli_response_schema_root!(
        "graph-neighbors",
        "graph neighbors",
        CliGraphNeighborsOutput
    ),
    cli_response_schema_root!(
        "graph-neighborhood",
        "graph neighborhood",
        CliGraphNeighborhoodOutput
    ),
    cli_response_schema_root!("graph-map", "graph map", CliGraphMapOutput),
    cli_response_schema_root!("graph-query", "graph query", CliGraphQueryOutput),
    cli_response_schema_root!("graph-rebuild", "graph rebuild", CliGraphRebuildOutput),
    cli_response_schema_root!("graph-status", "graph status", CliGraphStatusOutput),
    cli_response_schema_root!("graph-sync", "graph sync", CliGraphSyncOutput),
    cli_response_schema_root!(
        "vector-configure",
        "vector configure",
        CliVectorConfigureOutput
    ),
    cli_response_schema_root!(
        "vector-query-chunks",
        "vector query-chunks",
        CliVectorQueryChunksOutput
    ),
    cli_response_schema_root!(
        "vector-query-label-atoms",
        "vector query-label-atoms",
        CliVectorQueryLabelAtomsOutput
    ),
    cli_response_schema_root!("vector-rebuild", "vector rebuild", CliVectorRebuildOutput),
    cli_response_schema_root!("vector-status", "vector status", CliVectorStatusOutput),
    cli_response_schema_root!("vector-sync", "vector sync", CliVectorSyncOutput),
    cli_response_schema_root!("context-build", "context build", CliContextBuildOutput),
    cli_response_schema_root!("search", "search", CliSearchOutput),
    cli_response_schema_root!("index-rebuild", "index rebuild", CliIndexRebuildOutput),
    cli_response_schema_root!("index-sync", "index sync", CliIndexSyncOutput),
    cli_response_schema_root!("signal-confirm", "signal confirm", CliSignalConfirmOutput),
    cli_response_schema_root!("signal-list", "signal list", CliSignalListOutput),
    cli_response_schema_root!("signal-record", "signal record", CliSignalRecordOutput),
    cli_response_schema_root!("signal-reject", "signal reject", CliSignalRejectOutput),
    cli_response_schema_root!("signal-resolve", "signal resolve", CliSignalResolveOutput),
    cli_response_schema_root!("signal-review", "signal review", CliSignalReviewOutput),
    cli_response_schema_root!("signal-show", "signal show", CliSignalShowOutput),
    cli_response_schema_root!(
        "signal-supersede",
        "signal supersede",
        CliSignalSupersedeOutput
    ),
    cli_response_schema_root!(
        "hook-codex-install",
        "hook codex install",
        CliHookCodexInstallOutput
    ),
    cli_response_schema_root!(
        "hook-codex-status",
        "hook codex status",
        CliHookCodexStatusOutput
    ),
    cli_response_schema_root!(
        "hook-codex-uninstall",
        "hook codex uninstall",
        CliHookCodexUninstallOutput
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-stats-query:v1",
        "api/get-stats-query.v1.schema.json",
        "Kanban get stats query v1",
        "api.get-stats.query",
        "schemas/fixtures/api/get-stats-query.v1.valid.json",
        "schemas/fixtures/api/get-stats-query.v1.invalid.json",
        BoardQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-stats-response:v1",
        "api/get-stats-response.v1.schema.json",
        "Kanban get stats response v1",
        "api.get-stats.response",
        "schemas/fixtures/api/get-stats-response.v1.valid.json",
        "schemas/fixtures/api/get-stats-response.v1.invalid.json",
        StatsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:search-tasks-query:v1",
        "api/search-tasks-query.v1.schema.json",
        "Kanban search tasks query v1",
        "api.search-tasks.query",
        "schemas/fixtures/api/search-tasks-query.v1.valid.json",
        "schemas/fixtures/api/search-tasks-query.v1.invalid.json",
        SearchTasksQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:search-tasks-response:v1",
        "api/search-tasks-response.v1.schema.json",
        "Kanban search tasks response v1",
        "api.search-tasks.response",
        "schemas/fixtures/api/search-tasks-response.v1.valid.json",
        "schemas/fixtures/api/search-tasks-response.v1.invalid.json",
        SearchTasksResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:search-tasks-by-status-query:v1",
        "api/search-tasks-by-status-query.v1.schema.json",
        "Kanban search tasks by status query v1",
        "api.search-tasks-by-status.query",
        "schemas/fixtures/api/search-tasks-by-status-query.v1.valid.json",
        "schemas/fixtures/api/search-tasks-by-status-query.v1.invalid.json",
        SearchTasksQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:search-tasks-by-status-response:v1",
        "api/search-tasks-by-status-response.v1.schema.json",
        "Kanban search tasks by status response v1",
        "api.search-tasks-by-status.response",
        "schemas/fixtures/api/search-tasks-by-status-response.v1.valid.json",
        "schemas/fixtures/api/search-tasks-by-status-response.v1.invalid.json",
        SearchTasksByStatusResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:search-status-query:v1",
        "api/search-status-query.v1.schema.json",
        "Kanban search status query v1",
        "api.search-status.query",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "schemas/fixtures/api/search-status-query.v1.invalid.json",
        BoardQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:search-status-response:v1",
        "api/search-status-response.v1.schema.json",
        "Kanban search status response v1",
        "api.search-status.response",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "schemas/fixtures/api/search-status-response.v1.invalid.json",
        SearchStatusResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:rebuild-search-index-query:v1",
        "api/rebuild-search-index-query.v1.schema.json",
        "Kanban rebuild search index query v1",
        "api.rebuild-search-index.query",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "schemas/fixtures/api/search-status-query.v1.invalid.json",
        BoardQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:rebuild-search-index-response:v1",
        "api/rebuild-search-index-response.v1.schema.json",
        "Kanban rebuild search index response v1",
        "api.rebuild-search-index.response",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "schemas/fixtures/api/search-status-response.v1.invalid.json",
        SearchStatusResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:sync-search-index-query:v1",
        "api/sync-search-index-query.v1.schema.json",
        "Kanban sync search index query v1",
        "api.sync-search-index.query",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "schemas/fixtures/api/search-status-query.v1.invalid.json",
        BoardQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:sync-search-index-response:v1",
        "api/sync-search-index-response.v1.schema.json",
        "Kanban sync search index response v1",
        "api.sync-search-index.response",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "schemas/fixtures/api/search-status-response.v1.invalid.json",
        SearchStatusResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:build-context-path:v1",
        "api/build-context-path.v1.schema.json",
        "Kanban build context path v1",
        "api.build-context.path",
        "schemas/fixtures/api/build-context-path.v1.valid.json",
        "schemas/fixtures/api/build-context-path.v1.invalid.json",
        BuildContextPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:build-context-query:v1",
        "api/build-context-query.v1.schema.json",
        "Kanban build context query v1",
        "api.build-context.query",
        "schemas/fixtures/api/build-context-query.v1.valid.json",
        "schemas/fixtures/api/build-context-query.v1.invalid.json",
        BuildContextQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:build-context-response:v1",
        "api/build-context-response.v1.schema.json",
        "Kanban build context response v1",
        "api.build-context.response",
        "schemas/fixtures/api/build-context-response.v1.valid.json",
        "schemas/fixtures/api/build-context-response.v1.invalid.json",
        BuildContextResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:graph-status-query:v1",
        "api/graph-status-query.v1.schema.json",
        "Kanban graph status query v1",
        "api.graph-status.query",
        "schemas/fixtures/api/graph-status-query.v1.valid.json",
        "schemas/fixtures/api/graph-status-query.v1.invalid.json",
        BoardQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:graph-rebuild-query:v1",
        "api/graph-rebuild-query.v1.schema.json",
        "Kanban graph rebuild query v1",
        "api.graph-rebuild.query",
        "schemas/fixtures/api/graph-rebuild-query.v1.valid.json",
        "schemas/fixtures/api/graph-rebuild-query.v1.invalid.json",
        BoardQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:graph-sync-query:v1",
        "api/graph-sync-query.v1.schema.json",
        "Kanban graph sync query v1",
        "api.graph-sync.query",
        "schemas/fixtures/api/graph-sync-query.v1.valid.json",
        "schemas/fixtures/api/graph-sync-query.v1.invalid.json",
        BoardQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:graph-status-response:v1",
        "api/graph-status-response.v1.schema.json",
        "Kanban graph status response v1",
        "api.graph-status.response",
        "schemas/fixtures/api/graph-status-response.v1.valid.json",
        "schemas/fixtures/api/graph-status-response.v1.invalid.json",
        GraphStatusResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:graph-neighbors-query:v1",
        "api/graph-neighbors-query.v1.schema.json",
        "Kanban graph neighbors query v1",
        "api.graph-neighbors.query",
        "schemas/fixtures/api/graph-neighbors-query.v1.valid.json",
        "schemas/fixtures/api/graph-neighbors-query.v1.invalid.json",
        GraphNeighborsQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:graph-neighbors-response:v1",
        "api/graph-neighbors-response.v1.schema.json",
        "Kanban graph neighbors response v1",
        "api.graph-neighbors.response",
        "schemas/fixtures/api/graph-neighbors-response.v1.valid.json",
        "schemas/fixtures/api/graph-neighbors-response.v1.invalid.json",
        GraphNeighborsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:graph-query-query:v1",
        "api/graph-query-query.v1.schema.json",
        "Kanban graph query query v1",
        "api.graph-query.query",
        "schemas/fixtures/api/graph-query-query.v1.valid.json",
        "schemas/fixtures/api/graph-query-query.v1.invalid.json",
        GraphQueryQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:graph-query-response:v1",
        "api/graph-query-response.v1.schema.json",
        "Kanban graph query response v1",
        "api.graph-query.response",
        "schemas/fixtures/api/graph-query-response.v1.valid.json",
        "schemas/fixtures/api/graph-query-response.v1.invalid.json",
        CliGraphQueryOutput
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:graph-rebuild-response:v1",
        "api/graph-rebuild-response.v1.schema.json",
        "Kanban graph rebuild response v1",
        "api.graph-rebuild.response",
        "schemas/fixtures/api/graph-rebuild-response.v1.valid.json",
        "schemas/fixtures/api/graph-rebuild-response.v1.invalid.json",
        GraphMaintenanceResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:graph-sync-response:v1",
        "api/graph-sync-response.v1.schema.json",
        "Kanban graph sync response v1",
        "api.graph-sync.response",
        "schemas/fixtures/api/graph-sync-response.v1.valid.json",
        "schemas/fixtures/api/graph-sync-response.v1.invalid.json",
        GraphMaintenanceResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:entity-list-query:v1",
        "api/entity-list-query.v1.schema.json",
        "Kanban entity list query v1",
        "api.entity-list.query",
        "schemas/fixtures/api/entity-list-query.v1.valid.json",
        "schemas/fixtures/api/entity-list-query.v1.invalid.json",
        EntityListQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:entity-path:v1",
        "api/entity-path.v1.schema.json",
        "Kanban entity path v1",
        "api.entity.path",
        "schemas/fixtures/api/entity-path.v1.valid.json",
        "schemas/fixtures/api/entity-path.v1.invalid.json",
        EntityPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:entity-upsert-request:v1",
        "api/entity-upsert-request.v1.schema.json",
        "Kanban entity upsert request v1",
        "api.entity-upsert.request",
        "schemas/fixtures/api/entity-upsert-request.v1.valid.json",
        "schemas/fixtures/api/entity-upsert-request.v1.invalid.json",
        EntityUpsertRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:entity-list-response:v1",
        "api/entity-list-response.v1.schema.json",
        "Kanban entity list response v1",
        "api.entity-list.response",
        "schemas/fixtures/api/entity-list-response.v1.valid.json",
        "schemas/fixtures/api/entity-list-response.v1.invalid.json",
        EntityListResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:entity-response:v1",
        "api/entity-response.v1.schema.json",
        "Kanban entity response v1",
        "api.entity.response",
        "schemas/fixtures/api/entity-response.v1.valid.json",
        "schemas/fixtures/api/entity-response.v1.invalid.json",
        EntityResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:entity-upsert-response:v1",
        "api/entity-upsert-response.v1.schema.json",
        "Kanban entity upsert response v1",
        "api.entity-upsert.response",
        "schemas/fixtures/api/entity-upsert-response.v1.valid.json",
        "schemas/fixtures/api/entity-upsert-response.v1.invalid.json",
        EntityResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:vector-status-query:v1",
        "api/vector-status-query.v1.schema.json",
        "Kanban vector status query v1",
        "api.vector-status.query",
        "schemas/fixtures/api/vector-status-query.v1.valid.json",
        "schemas/fixtures/api/vector-status-query.v1.invalid.json",
        VectorStatusQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:vector-status-response:v1",
        "api/vector-status-response.v1.schema.json",
        "Kanban vector status response v1",
        "api.vector-status.response",
        "schemas/fixtures/api/vector-status-response.v1.valid.json",
        "schemas/fixtures/api/vector-status-response.v1.invalid.json",
        VectorStatusResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:vector-configure-request:v1",
        "api/vector-configure-request.v1.schema.json",
        "Kanban vector configure request v1",
        "api.vector-configure.request",
        "schemas/fixtures/api/vector-configure-request.v1.valid.json",
        "schemas/fixtures/api/vector-configure-request.v1.invalid.json",
        VectorConfigureRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:vector-configure-response:v1",
        "api/vector-configure-response.v1.schema.json",
        "Kanban vector configure response v1",
        "api.vector-configure.response",
        "schemas/fixtures/api/vector-configure-response.v1.valid.json",
        "schemas/fixtures/api/vector-configure-response.v1.invalid.json",
        VectorConfigureResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:vector-rebuild-request:v1",
        "api/vector-rebuild-request.v1.schema.json",
        "Kanban vector rebuild request v1",
        "api.vector-rebuild.request",
        "schemas/fixtures/api/vector-rebuild-request.v1.valid.json",
        "schemas/fixtures/api/vector-rebuild-request.v1.invalid.json",
        VectorProjectionRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:vector-rebuild-response:v1",
        "api/vector-rebuild-response.v1.schema.json",
        "Kanban vector rebuild response v1",
        "api.vector-rebuild.response",
        "schemas/fixtures/api/vector-rebuild-response.v1.valid.json",
        "schemas/fixtures/api/vector-rebuild-response.v1.invalid.json",
        VectorProjectionResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:vector-sync-request:v1",
        "api/vector-sync-request.v1.schema.json",
        "Kanban vector sync request v1",
        "api.vector-sync.request",
        "schemas/fixtures/api/vector-sync-request.v1.valid.json",
        "schemas/fixtures/api/vector-sync-request.v1.invalid.json",
        VectorProjectionRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:vector-sync-response:v1",
        "api/vector-sync-response.v1.schema.json",
        "Kanban vector sync response v1",
        "api.vector-sync.response",
        "schemas/fixtures/api/vector-sync-response.v1.valid.json",
        "schemas/fixtures/api/vector-sync-response.v1.invalid.json",
        VectorProjectionResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:vector-query-chunks-query:v1",
        "api/vector-query-chunks-query.v1.schema.json",
        "Kanban vector query chunks query v1",
        "api.vector-query-chunks.query",
        "schemas/fixtures/api/vector-query-chunks-query.v1.valid.json",
        "schemas/fixtures/api/vector-query-chunks-query.v1.invalid.json",
        VectorQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:vector-query-chunks-response:v1",
        "api/vector-query-chunks-response.v1.schema.json",
        "Kanban vector query chunks response v1",
        "api.vector-query-chunks.response",
        "schemas/fixtures/api/vector-query-chunks-response.v1.valid.json",
        "schemas/fixtures/api/vector-query-chunks-response.v1.invalid.json",
        VectorQueryChunksResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:vector-query-label-atoms-query:v1",
        "api/vector-query-label-atoms-query.v1.schema.json",
        "Kanban vector query label atoms query v1",
        "api.vector-query-label-atoms.query",
        "schemas/fixtures/api/vector-query-label-atoms-query.v1.valid.json",
        "schemas/fixtures/api/vector-query-label-atoms-query.v1.invalid.json",
        VectorQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:vector-query-label-atoms-response:v1",
        "api/vector-query-label-atoms-response.v1.schema.json",
        "Kanban vector query label atoms response v1",
        "api.vector-query-label-atoms.response",
        "schemas/fixtures/api/vector-query-label-atoms-response.v1.valid.json",
        "schemas/fixtures/api/vector-query-label-atoms-response.v1.invalid.json",
        VectorQueryLabelAtomsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-events-query:v1",
        "api/list-events-query.v1.schema.json",
        "Kanban list events query v1",
        "api.list-events.query",
        "schemas/fixtures/api/list-events-query.v1.valid.json",
        "schemas/fixtures/api/list-events-query.v1.invalid.json",
        ListEventsQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-label-ontology-signals-response:v1",
        "api/list-label-ontology-signals-response.v1.schema.json",
        "Kanban API list label ontology signals response v1",
        "api.list-label-ontology-signals.response",
        "schemas/fixtures/api/list-label-ontology-signals-response.v1.valid.json",
        "schemas/fixtures/api/list-label-ontology-signals-response.v1.invalid.json",
        LabelOntologySignalsResponse
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:doctor-response:v1",
        artifact_path: "api/doctor-response.v1.schema.json",
        title: "Kanban doctor response v1",
        contract_id: "api.doctor.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/doctor-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/doctor-response.v1.invalid.json",
        generate: generate_for::<DoctorResponse>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:checkpoint-response:v1",
        artifact_path: "api/checkpoint-response.v1.schema.json",
        title: "Kanban checkpoint response v1",
        contract_id: "api.checkpoint.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/checkpoint-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/checkpoint-response.v1.invalid.json",
        generate: generate_for::<CheckpointResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-path-request:v1",
        "api/maintenance-path-request.v1.schema.json",
        "Kanban maintenance path request v1",
        "api.maintenance-path.request",
        "schemas/fixtures/api/maintenance-path-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-path-request.v1.invalid.json",
        MaintenancePathRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-backup-request:v1",
        "api/maintenance-backup-request.v1.schema.json",
        "Kanban maintenance backup request v1",
        "api.maintenance-backup.request",
        "schemas/fixtures/api/maintenance-backup-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-backup-request.v1.invalid.json",
        MaintenancePathRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-export-request:v1",
        "api/maintenance-export-request.v1.schema.json",
        "Kanban maintenance export request v1",
        "api.maintenance-export.request",
        "schemas/fixtures/api/maintenance-export-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-export-request.v1.invalid.json",
        MaintenancePathRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-import-request:v1",
        "api/maintenance-import-request.v1.schema.json",
        "Kanban maintenance import request v1",
        "api.maintenance-import.request",
        "schemas/fixtures/api/maintenance-import-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-request.v1.invalid.json",
        MaintenanceImportRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-import-v30-request:v1",
        "api/maintenance-import-v30-request.v1.schema.json",
        "Kanban legacy SQLite v30 import request v1",
        "api.maintenance-import-v30.request",
        "schemas/fixtures/api/maintenance-import-v30-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-v30-request.v1.invalid.json",
        LegacyImportRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-run-request:v1",
        "api/maintenance-run-request.v1.schema.json",
        "Kanban maintenance run request v1",
        "api.maintenance-run.request",
        "schemas/fixtures/api/maintenance-run-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-run-request.v1.invalid.json",
        MaintenanceRunRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-rebuild-request:v1",
        "api/maintenance-rebuild-request.v1.schema.json",
        "Kanban maintenance rebuild request v1",
        "api.maintenance-rebuild.request",
        "schemas/fixtures/api/maintenance-rebuild-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-rebuild-request.v1.invalid.json",
        MaintenanceRunRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-cleanup-request:v1",
        "api/maintenance-cleanup-request.v1.schema.json",
        "Kanban maintenance cleanup request v1",
        "api.maintenance-cleanup.request",
        "schemas/fixtures/api/maintenance-cleanup-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-cleanup-request.v1.invalid.json",
        MaintenanceRunRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-backup-response:v1",
        "api/maintenance-backup-response.v1.schema.json",
        "Kanban maintenance backup response v1",
        "api.maintenance-backup.response",
        "schemas/fixtures/api/maintenance-backup-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-backup-response.v1.invalid.json",
        BackupResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-export-response:v1",
        "api/maintenance-export-response.v1.schema.json",
        "Kanban maintenance export response v1",
        "api.maintenance-export.response",
        "schemas/fixtures/api/maintenance-export-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-export-response.v1.invalid.json",
        ExportResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-import-response:v1",
        "api/maintenance-import-response.v1.schema.json",
        "Kanban maintenance import response v1",
        "api.maintenance-import.response",
        "schemas/fixtures/api/maintenance-import-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-response.v1.invalid.json",
        ImportResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-import-v30-response:v1",
        "api/maintenance-import-v30-response.v1.schema.json",
        "Kanban legacy SQLite v30 import response v1",
        "api.maintenance-import-v30.response",
        "schemas/fixtures/api/maintenance-import-v30-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-v30-response.v1.invalid.json",
        LegacyImportResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-vacuum-response:v1",
        "api/maintenance-vacuum-response.v1.schema.json",
        "Kanban maintenance vacuum response v1",
        "api.maintenance-vacuum.response",
        "schemas/fixtures/api/maintenance-vacuum-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-vacuum-response.v1.invalid.json",
        VacuumResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-status-response:v1",
        "api/maintenance-status-response.v1.schema.json",
        "Kanban maintenance status response v1",
        "api.maintenance-status.response",
        "schemas/fixtures/api/maintenance-status-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-status-response.v1.invalid.json",
        MaintenanceStatusResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-run-response:v1",
        "api/maintenance-run-response.v1.schema.json",
        "Kanban maintenance run response v1",
        "api.maintenance-run.response",
        "schemas/fixtures/api/maintenance-run-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-run-response.v1.invalid.json",
        MaintenanceRunResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-rebuild-response:v1",
        "api/maintenance-rebuild-response.v1.schema.json",
        "Kanban maintenance rebuild response v1",
        "api.maintenance-rebuild.response",
        "schemas/fixtures/api/maintenance-rebuild-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-rebuild-response.v1.invalid.json",
        MaintenanceRunResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:maintenance-cleanup-response:v1",
        "api/maintenance-cleanup-response.v1.schema.json",
        "Kanban maintenance cleanup response v1",
        "api.maintenance-cleanup.response",
        "schemas/fixtures/api/maintenance-cleanup-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-cleanup-response.v1.invalid.json",
        MaintenanceRunResponse
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:error-response:v1",
        artifact_path: "api/error-response.v1.schema.json",
        title: "Kanban API error response v1",
        contract_id: "api.error.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/error-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/error-response.v1.invalid.json",
        generate: generate_for::<ErrorEnvelope>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:health-response:v1",
        artifact_path: "api/health-response.v1.schema.json",
        title: "Kanban API health response v1",
        contract_id: "api.health.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/health-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/health-response.v1.invalid.json",
        generate: generate_for::<HealthResponse>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:list-tasks-response:v1",
        artifact_path: "api/list-tasks-response.v1.schema.json",
        title: "Kanban list tasks response v1",
        contract_id: "api.list-tasks.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/list-tasks-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/list-tasks-response.v1.invalid.json",
        generate: generate_for::<ListTasksResponse>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:list-tasks-by-status-response:v1",
        artifact_path: "api/list-tasks-by-status-response.v1.schema.json",
        title: "Kanban list tasks by status response v1",
        contract_id: "api.list-tasks-by-status.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/list-tasks-by-status-response.v1.invalid.json",
        generate: generate_for::<ListTasksByStatusResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-task-path:v1",
        "api/create-task-path.v1.schema.json",
        "Kanban create task path v1",
        "api.create-task.path",
        "schemas/fixtures/api/create-task-path.v1.valid.json",
        "schemas/fixtures/api/create-task-path.v1.invalid.json",
        CreateTaskPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-task-request:v1",
        "api/create-task-request.v1.schema.json",
        "Kanban create task request v1",
        "api.create-task.request",
        "schemas/fixtures/api/create-task-request.v1.valid.json",
        "schemas/fixtures/api/create-task-request.v1.invalid.json",
        CreateTaskRequest
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:create-task-response:v1",
        artifact_path: "api/create-task-response.v1.schema.json",
        title: "Kanban create task response v1",
        contract_id: "api.create-task.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/create-task-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/create-task-response.v1.invalid.json",
        generate: generate_for::<CreateTaskResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-tasks-path:v1",
        "api/list-tasks-path.v1.schema.json",
        "Kanban list tasks path v1",
        "api.list-tasks.path",
        "schemas/fixtures/api/list-tasks-path.v1.valid.json",
        "schemas/fixtures/api/list-tasks-path.v1.invalid.json",
        ListTasksPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-tasks-query:v1",
        "api/list-tasks-query.v1.schema.json",
        "Kanban list tasks query v1",
        "api.list-tasks.query",
        "schemas/fixtures/api/list-tasks-query.v1.valid.json",
        "schemas/fixtures/api/list-tasks-query.v1.invalid.json",
        ListTasksQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-tasks-by-status-path:v1",
        "api/list-tasks-by-status-path.v1.schema.json",
        "Kanban list tasks by status path v1",
        "api.list-tasks-by-status.path",
        "schemas/fixtures/api/list-tasks-by-status-path.v1.valid.json",
        "schemas/fixtures/api/list-tasks-by-status-path.v1.invalid.json",
        ListTasksByStatusPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-tasks-by-status-query:v1",
        "api/list-tasks-by-status-query.v1.schema.json",
        "Kanban list tasks by status query v1",
        "api.list-tasks-by-status.query",
        "schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json",
        "schemas/fixtures/api/list-tasks-by-status-query.v1.invalid.json",
        ListTasksByStatusQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-runs-path:v1",
        "api/list-runs-path.v1.schema.json",
        "Kanban list runs path v1",
        "api.list-runs.path",
        "schemas/fixtures/api/list-runs-path.v1.valid.json",
        "schemas/fixtures/api/list-runs-path.v1.invalid.json",
        ListRunsPath
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:list-runs-response:v1",
        artifact_path: "api/list-runs-response.v1.schema.json",
        title: "Kanban list runs response v1",
        contract_id: "api.list-runs.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/list-runs-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/list-runs-response.v1.invalid.json",
        generate: generate_for::<ListRunsResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-run-path:v1",
        "api/get-run-path.v1.schema.json",
        "Kanban get run path v1",
        "api.get-run.path",
        "schemas/fixtures/api/get-run-path.v1.valid.json",
        "schemas/fixtures/api/get-run-path.v1.invalid.json",
        GetRunPath
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:get-run-response:v1",
        artifact_path: "api/get-run-response.v1.schema.json",
        title: "Kanban get run response v1",
        contract_id: "api.get-run.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/get-run-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/get-run-response.v1.invalid.json",
        generate: generate_for::<GetRunResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-comments-path:v1",
        "api/list-comments-path.v1.schema.json",
        "Kanban list comments path v1",
        "api.list-comments.path",
        "schemas/fixtures/api/list-comments-path.v1.valid.json",
        "schemas/fixtures/api/list-comments-path.v1.invalid.json",
        ListCommentsPath
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:list-comments-response:v1",
        artifact_path: "api/list-comments-response.v1.schema.json",
        title: "Kanban list comments response v1",
        contract_id: "api.list-comments.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/list-comments-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/list-comments-response.v1.invalid.json",
        generate: generate_for::<ListCommentsResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-comment-path:v1",
        "api/create-comment-path.v1.schema.json",
        "Kanban create comment path v1",
        "api.create-comment.path",
        "schemas/fixtures/api/create-comment-path.v1.valid.json",
        "schemas/fixtures/api/create-comment-path.v1.invalid.json",
        CreateCommentPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-comment-request:v1",
        "api/create-comment-request.v1.schema.json",
        "Kanban create comment request v1",
        "api.create-comment.request",
        "schemas/fixtures/api/create-comment-request.v1.valid.json",
        "schemas/fixtures/api/create-comment-request.v1.invalid.json",
        CreateCommentRequest
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:create-comment-response:v1",
        artifact_path: "api/create-comment-response.v1.schema.json",
        title: "Kanban create comment response v1",
        contract_id: "api.create-comment.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/create-comment-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/create-comment-response.v1.invalid.json",
        generate: generate_for::<CreateCommentResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-steps-path:v1",
        "api/list-steps-path.v1.schema.json",
        "Kanban list steps path v1",
        "api.list-steps.path",
        "schemas/fixtures/api/list-steps-path.v1.valid.json",
        "schemas/fixtures/api/list-steps-path.v1.invalid.json",
        ListStepsPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-steps-response:v1",
        "api/list-steps-response.v1.schema.json",
        "Kanban list steps response v1",
        "api.list-steps.response",
        "schemas/fixtures/api/list-steps-response.v1.valid.json",
        "schemas/fixtures/api/list-steps-response.v1.invalid.json",
        ListStepsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-step-path:v1",
        "api/create-step-path.v1.schema.json",
        "Kanban create step path v1",
        "api.create-step.path",
        "schemas/fixtures/api/create-step-path.v1.valid.json",
        "schemas/fixtures/api/create-step-path.v1.invalid.json",
        CreateStepPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-step-request:v1",
        "api/create-step-request.v1.schema.json",
        "Kanban create step request v1",
        "api.create-step.request",
        "schemas/fixtures/api/create-step-request.v1.valid.json",
        "schemas/fixtures/api/create-step-request.v1.invalid.json",
        CreateStepRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:create-step-response:v1",
        "api/create-step-response.v1.schema.json",
        "Kanban create step response v1",
        "api.create-step.response",
        "schemas/fixtures/api/create-step-response.v1.valid.json",
        "schemas/fixtures/api/create-step-response.v1.invalid.json",
        CreateStepResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:update-step-path:v1",
        "api/update-step-path.v1.schema.json",
        "Kanban update step path v1",
        "api.update-step.path",
        "schemas/fixtures/api/update-step-path.v1.valid.json",
        "schemas/fixtures/api/update-step-path.v1.invalid.json",
        UpdateStepPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:update-step-request:v1",
        "api/update-step-request.v1.schema.json",
        "Kanban update step request v1",
        "api.update-step.request",
        "schemas/fixtures/api/update-step-request.v1.valid.json",
        "schemas/fixtures/api/update-step-request.v1.invalid.json",
        UpdateStepRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:update-step-response:v1",
        "api/update-step-response.v1.schema.json",
        "Kanban update step response v1",
        "api.update-step.response",
        "schemas/fixtures/api/update-step-response.v1.valid.json",
        "schemas/fixtures/api/update-step-response.v1.invalid.json",
        UpdateStepResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:remove-step-path:v1",
        "api/remove-step-path.v1.schema.json",
        "Kanban remove step path v1",
        "api.remove-step.path",
        "schemas/fixtures/api/remove-step-path.v1.valid.json",
        "schemas/fixtures/api/remove-step-path.v1.invalid.json",
        RemoveStepPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:remove-step-response:v1",
        "api/remove-step-response.v1.schema.json",
        "Kanban remove step response v1",
        "api.remove-step.response",
        "schemas/fixtures/api/remove-step-response.v1.valid.json",
        "schemas/fixtures/api/remove-step-response.v1.invalid.json",
        RemoveStepResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:complete-step-path:v1",
        "api/complete-step-path.v1.schema.json",
        "Kanban complete step path v1",
        "api.complete-step.path",
        "schemas/fixtures/api/complete-step-path.v1.valid.json",
        "schemas/fixtures/api/complete-step-path.v1.invalid.json",
        CompleteStepPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:complete-step-request:v1",
        "api/complete-step-request.v1.schema.json",
        "Kanban complete step request v1",
        "api.complete-step.request",
        "schemas/fixtures/api/complete-step-request.v1.valid.json",
        "schemas/fixtures/api/complete-step-request.v1.invalid.json",
        CompleteStepRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:complete-step-response:v1",
        "api/complete-step-response.v1.schema.json",
        "Kanban complete step response v1",
        "api.complete-step.response",
        "schemas/fixtures/api/complete-step-response.v1.valid.json",
        "schemas/fixtures/api/complete-step-response.v1.invalid.json",
        CompleteStepResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:skip-step-path:v1",
        "api/skip-step-path.v1.schema.json",
        "Kanban skip step path v1",
        "api.skip-step.path",
        "schemas/fixtures/api/skip-step-path.v1.valid.json",
        "schemas/fixtures/api/skip-step-path.v1.invalid.json",
        SkipStepPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:skip-step-request:v1",
        "api/skip-step-request.v1.schema.json",
        "Kanban skip step request v1",
        "api.skip-step.request",
        "schemas/fixtures/api/skip-step-request.v1.valid.json",
        "schemas/fixtures/api/skip-step-request.v1.invalid.json",
        SkipStepRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:skip-step-response:v1",
        "api/skip-step-response.v1.schema.json",
        "Kanban skip step response v1",
        "api.skip-step.response",
        "schemas/fixtures/api/skip-step-response.v1.valid.json",
        "schemas/fixtures/api/skip-step-response.v1.invalid.json",
        SkipStepResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reopen-step-path:v1",
        "api/reopen-step-path.v1.schema.json",
        "Kanban reopen step path v1",
        "api.reopen-step.path",
        "schemas/fixtures/api/reopen-step-path.v1.valid.json",
        "schemas/fixtures/api/reopen-step-path.v1.invalid.json",
        ReopenStepPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reopen-step-request:v1",
        "api/reopen-step-request.v1.schema.json",
        "Kanban reopen step request v1",
        "api.reopen-step.request",
        "schemas/fixtures/api/reopen-step-request.v1.valid.json",
        "schemas/fixtures/api/reopen-step-request.v1.invalid.json",
        ReopenStepRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:reopen-step-response:v1",
        "api/reopen-step-response.v1.schema.json",
        "Kanban reopen step response v1",
        "api.reopen-step.response",
        "schemas/fixtures/api/reopen-step-response.v1.valid.json",
        "schemas/fixtures/api/reopen-step-response.v1.invalid.json",
        ReopenStepResponse
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:remove-task-label-response:v1",
        artifact_path: "api/remove-task-label-response.v1.schema.json",
        title: "Kanban remove task label response v1",
        contract_id: "api.remove-task-label.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/remove-task-label-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/remove-task-label-response.v1.invalid.json",
        generate: generate_for::<RemoveTaskLabelResponse>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:add-task-label-response:v1",
        artifact_path: "api/add-task-label-response.v1.schema.json",
        title: "Kanban add task label response v1",
        contract_id: "api.add-task-label.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/add-task-label-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/add-task-label-response.v1.invalid.json",
        generate: generate_for::<AddTaskLabelResponse>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:list-task-labels-response:v1",
        artifact_path: "api/list-task-labels-response.v1.schema.json",
        title: "Kanban list task labels response v1",
        contract_id: "api.list-task-labels.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/list-task-labels-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/list-task-labels-response.v1.invalid.json",
        generate: generate_for::<ListTaskLabelsResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:add-task-label-request:v1",
        "api/add-task-label-request.v1.schema.json",
        "Kanban add task label request v1",
        "api.add-task-label.request",
        "schemas/fixtures/api/add-task-label-request.v1.valid.json",
        "schemas/fixtures/api/add-task-label-request.v1.invalid.json",
        AddTaskLabelRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:remove-task-label-path:v1",
        "api/remove-task-label-path.v1.schema.json",
        "Kanban remove task label path v1",
        "api.remove-task-label.path",
        "schemas/fixtures/api/remove-task-label-path.v1.valid.json",
        "schemas/fixtures/api/remove-task-label-path.v1.invalid.json",
        RemoveTaskLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:add-task-label-path:v1",
        "api/add-task-label-path.v1.schema.json",
        "Kanban add task label path v1",
        "api.add-task-label.path",
        "schemas/fixtures/api/add-task-label-path.v1.valid.json",
        "schemas/fixtures/api/add-task-label-path.v1.invalid.json",
        AddTaskLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-task-labels-path:v1",
        "api/list-task-labels-path.v1.schema.json",
        "Kanban task labels path v1",
        "api.list-task-labels.path",
        "schemas/fixtures/api/list-task-labels-path.v1.valid.json",
        "schemas/fixtures/api/list-task-labels-path.v1.invalid.json",
        ListTaskLabelsPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-task-path:v1",
        "api/get-task-path.v1.schema.json",
        "Kanban get task path v1",
        "api.get-task.path",
        "schemas/fixtures/api/get-task-path.v1.valid.json",
        "schemas/fixtures/api/get-task-path.v1.invalid.json",
        GetTaskPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-task-query:v1",
        "api/get-task-query.v1.schema.json",
        "Kanban get task query v1",
        "api.get-task.query",
        "schemas/fixtures/api/get-task-query.v1.valid.json",
        "schemas/fixtures/api/get-task-query.v1.invalid.json",
        GetTaskQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-task-response:v1",
        "api/get-task-response.v1.schema.json",
        "Kanban get task response v1",
        "api.get-task.response",
        "schemas/fixtures/api/get-task-response.v1.valid.json",
        "schemas/fixtures/api/get-task-response.v1.invalid.json",
        GetTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:update-task-path:v1",
        "api/update-task-path.v1.schema.json",
        "Kanban update task path v1",
        "api.update-task.path",
        "schemas/fixtures/api/update-task-path.v1.valid.json",
        "schemas/fixtures/api/update-task-path.v1.invalid.json",
        UpdateTaskPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:update-task-request:v1",
        "api/update-task-request.v1.schema.json",
        "Kanban update task request v1",
        "api.update-task.request",
        "schemas/fixtures/api/update-task-request.v1.valid.json",
        "schemas/fixtures/api/update-task-request.v1.invalid.json",
        UpdateTaskRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:update-task-response:v1",
        "api/update-task-response.v1.schema.json",
        "Kanban update task response v1",
        "api.update-task.response",
        "schemas/fixtures/api/update-task-response.v1.valid.json",
        "schemas/fixtures/api/update-task-response.v1.invalid.json",
        UpdateTaskResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:board-task-map-response:v1",
        "api/board-task-map-response.v1.schema.json",
        "Kanban board task map response v1",
        "api.board-task-map.response",
        "schemas/fixtures/api/board-task-map-response.v1.valid.json",
        "schemas/fixtures/api/board-task-map-response.v1.invalid.json",
        BoardTaskMapResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:task-neighborhood-response:v1",
        "api/task-neighborhood-response.v1.schema.json",
        "Kanban task neighborhood response v1",
        "api.task-neighborhood.response",
        "schemas/fixtures/api/task-neighborhood-response.v1.valid.json",
        "schemas/fixtures/api/task-neighborhood-response.v1.invalid.json",
        TaskNeighborhoodResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:board-task-map-path:v1",
        "api/board-task-map-path.v1.schema.json",
        "Kanban board task map path v1",
        "api.board-task-map.path",
        "schemas/fixtures/api/board-task-map-path.v1.valid.json",
        "schemas/fixtures/api/board-task-map-path.v1.invalid.json",
        BoardTaskMapPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:board-task-map-query:v1",
        "api/board-task-map-query.v1.schema.json",
        "Kanban board task map query v1",
        "api.board-task-map.query",
        "schemas/fixtures/api/board-task-map-query.v1.valid.json",
        "schemas/fixtures/api/board-task-map-query.v1.invalid.json",
        BoardTaskMapQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:task-neighborhood-path:v1",
        "api/task-neighborhood-path.v1.schema.json",
        "Kanban task neighborhood path v1",
        "api.task-neighborhood.path",
        "schemas/fixtures/api/task-neighborhood-path.v1.valid.json",
        "schemas/fixtures/api/task-neighborhood-path.v1.invalid.json",
        TaskNeighborhoodPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:task-neighborhood-query:v1",
        "api/task-neighborhood-query.v1.schema.json",
        "Kanban task neighborhood query v1",
        "api.task-neighborhood.query",
        "schemas/fixtures/api/task-neighborhood-query.v1.valid.json",
        "schemas/fixtures/api/task-neighborhood-query.v1.invalid.json",
        TaskNeighborhoodQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-events-response:v1",
        "api/list-events-response.v1.schema.json",
        "Kanban API list events response v1",
        "api.list-events.response",
        "schemas/fixtures/api/list-events-response.v1.valid.json",
        "schemas/fixtures/api/list-events-response.v1.invalid.json",
        ListEventsResponse
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:sse:stream-event-data:v1",
        artifact_path: "sse/stream-event-data.v1.schema.json",
        title: "Kanban SSE stream event data v1",
        contract_id: "sse.event.data",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/sse/stream-event-data.v1.valid.json",
        invalid_fixture: "schemas/fixtures/sse/stream-event-data.v1.invalid.json",
        generate: generate_for::<StreamEventData>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:sse:stream-events-query:v1",
        "sse/stream-events-query.v1.schema.json",
        "Kanban SSE stream events query v1",
        "sse.stream-events.query",
        "schemas/fixtures/sse/stream-events-query.v1.valid.json",
        "schemas/fixtures/sse/stream-events-query.v1.invalid.json",
        StreamEventsQuery
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:api:delete-response:v1",
        artifact_path: "api/delete-response.v1.schema.json",
        title: "Kanban API delete response v1",
        contract_id: "api.label-semantics-delete.response",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/delete-response.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/delete-response.v1.invalid.json",
        generate: generate_for::<DeleteResponse>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:specify-task-path:v1",
        "api/specify-task-path.v1.schema.json",
        "Kanban specify task path v1",
        "api.specify-task.path",
        "schemas/fixtures/api/specify-task-path.v1.valid.json",
        "schemas/fixtures/api/specify-task-path.v1.invalid.json",
        SpecifyTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:specify-task-response:v1",
        "api/specify-task-response.v1.schema.json",
        "Kanban specify task response v1",
        "api.specify-task.response",
        "schemas/fixtures/api/specify-task-response.v1.valid.json",
        "schemas/fixtures/api/specify-task-response.v1.invalid.json",
        SpecifyTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:promote-task-path:v1",
        "api/promote-task-path.v1.schema.json",
        "Kanban promote task path v1",
        "api.promote-task.path",
        "schemas/fixtures/api/promote-task-path.v1.valid.json",
        "schemas/fixtures/api/promote-task-path.v1.invalid.json",
        PromoteTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:promote-task-response:v1",
        "api/promote-task-response.v1.schema.json",
        "Kanban promote task response v1",
        "api.promote-task.response",
        "schemas/fixtures/api/promote-task-response.v1.valid.json",
        "schemas/fixtures/api/promote-task-response.v1.invalid.json",
        PromoteTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reopen-task-path:v1",
        "api/reopen-task-path.v1.schema.json",
        "Kanban reopen task path v1",
        "api.reopen-task.path",
        "schemas/fixtures/api/reopen-task-path.v1.valid.json",
        "schemas/fixtures/api/reopen-task-path.v1.invalid.json",
        ReopenTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:reopen-task-response:v1",
        "api/reopen-task-response.v1.schema.json",
        "Kanban reopen task response v1",
        "api.reopen-task.response",
        "schemas/fixtures/api/reopen-task-response.v1.valid.json",
        "schemas/fixtures/api/reopen-task-response.v1.invalid.json",
        ReopenTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:unblock-task-path:v1",
        "api/unblock-task-path.v1.schema.json",
        "Kanban unblock task path v1",
        "api.unblock-task.path",
        "schemas/fixtures/api/unblock-task-path.v1.valid.json",
        "schemas/fixtures/api/unblock-task-path.v1.invalid.json",
        UnblockTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:unblock-task-response:v1",
        "api/unblock-task-response.v1.schema.json",
        "Kanban unblock task response v1",
        "api.unblock-task.response",
        "schemas/fixtures/api/unblock-task-response.v1.valid.json",
        "schemas/fixtures/api/unblock-task-response.v1.invalid.json",
        UnblockTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:archive-task-path:v1",
        "api/archive-task-path.v1.schema.json",
        "Kanban archive task path v1",
        "api.archive-task.path",
        "schemas/fixtures/api/archive-task-path.v1.valid.json",
        "schemas/fixtures/api/archive-task-path.v1.invalid.json",
        ArchiveTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:archive-task-response:v1",
        "api/archive-task-response.v1.schema.json",
        "Kanban archive task response v1",
        "api.archive-task.response",
        "schemas/fixtures/api/archive-task-response.v1.valid.json",
        "schemas/fixtures/api/archive-task-response.v1.invalid.json",
        ArchiveTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:specify-task-request:v1",
        "api/specify-task-request.v1.schema.json",
        "Kanban specify task request v1",
        "api.specify-task.request",
        "schemas/fixtures/api/specify-task-request.v1.valid.json",
        "schemas/fixtures/api/specify-task-request.v1.invalid.json",
        SpecifyTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:promote-task-request:v1",
        "api/promote-task-request.v1.schema.json",
        "Kanban promote task request v1",
        "api.promote-task.request",
        "schemas/fixtures/api/promote-task-request.v1.valid.json",
        "schemas/fixtures/api/promote-task-request.v1.invalid.json",
        PromoteTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:claim-task-request:v1",
        "api/claim-task-request.v1.schema.json",
        "Kanban claim task request v1",
        "api.claim-task.request",
        "schemas/fixtures/api/claim-task-request.v1.valid.json",
        "schemas/fixtures/api/claim-task-request.v1.invalid.json",
        ClaimTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:claim-task-path:v1",
        "api/claim-task-path.v1.schema.json",
        "Kanban claim task path v1",
        "api.claim-task.path",
        "schemas/fixtures/api/claim-task-path.v1.valid.json",
        "schemas/fixtures/api/claim-task-path.v1.invalid.json",
        ClaimTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:claim-task-response:v1",
        "api/claim-task-response.v1.schema.json",
        "Kanban claim task response v1",
        "api.claim-task.response",
        "schemas/fixtures/api/claim-task-response.v1.valid.json",
        "schemas/fixtures/api/claim-task-response.v1.invalid.json",
        ClaimTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reclaim-task-request:v1",
        "api/reclaim-task-request.v1.schema.json",
        "Kanban reclaim task request v1",
        "api.reclaim-task.request",
        "schemas/fixtures/api/reclaim-task-request.v1.valid.json",
        "schemas/fixtures/api/reclaim-task-request.v1.invalid.json",
        ReclaimTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reclaim-task-path:v1",
        "api/reclaim-task-path.v1.schema.json",
        "Kanban reclaim task path v1",
        "api.reclaim-task.path",
        "schemas/fixtures/api/reclaim-task-path.v1.valid.json",
        "schemas/fixtures/api/reclaim-task-path.v1.invalid.json",
        ReclaimTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:reclaim-task-response:v1",
        "api/reclaim-task-response.v1.schema.json",
        "Kanban reclaim task response v1",
        "api.reclaim-task.response",
        "schemas/fixtures/api/reclaim-task-response.v1.valid.json",
        "schemas/fixtures/api/reclaim-task-response.v1.invalid.json",
        ReclaimTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:heartbeat-task-request:v1",
        "api/heartbeat-task-request.v1.schema.json",
        "Kanban heartbeat task request v1",
        "api.heartbeat-task.request",
        "schemas/fixtures/api/heartbeat-task-request.v1.valid.json",
        "schemas/fixtures/api/heartbeat-task-request.v1.invalid.json",
        HeartbeatTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:heartbeat-task-path:v1",
        "api/heartbeat-task-path.v1.schema.json",
        "Kanban heartbeat task path v1",
        "api.heartbeat-task.path",
        "schemas/fixtures/api/heartbeat-task-path.v1.valid.json",
        "schemas/fixtures/api/heartbeat-task-path.v1.invalid.json",
        HeartbeatTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:heartbeat-task-response:v1",
        "api/heartbeat-task-response.v1.schema.json",
        "Kanban heartbeat task response v1",
        "api.heartbeat-task.response",
        "schemas/fixtures/api/heartbeat-task-response.v1.valid.json",
        "schemas/fixtures/api/heartbeat-task-response.v1.invalid.json",
        HeartbeatTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:release-task-request:v1",
        "api/release-task-request.v1.schema.json",
        "Kanban release task request v1",
        "api.release-task.request",
        "schemas/fixtures/api/release-task-request.v1.valid.json",
        "schemas/fixtures/api/release-task-request.v1.invalid.json",
        ReleaseTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:release-task-path:v1",
        "api/release-task-path.v1.schema.json",
        "Kanban release task path v1",
        "api.release-task.path",
        "schemas/fixtures/api/release-task-path.v1.valid.json",
        "schemas/fixtures/api/release-task-path.v1.invalid.json",
        ReleaseTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:release-task-response:v1",
        "api/release-task-response.v1.schema.json",
        "Kanban release task response v1",
        "api.release-task.response",
        "schemas/fixtures/api/release-task-response.v1.valid.json",
        "schemas/fixtures/api/release-task-response.v1.invalid.json",
        ReleaseTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:complete-task-request:v1",
        "api/complete-task-request.v1.schema.json",
        "Kanban complete task request v1",
        "api.complete-task.request",
        "schemas/fixtures/api/complete-task-request.v1.valid.json",
        "schemas/fixtures/api/complete-task-request.v1.invalid.json",
        CompleteTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:complete-task-path:v1",
        "api/complete-task-path.v1.schema.json",
        "Kanban complete task path v1",
        "api.complete-task.path",
        "schemas/fixtures/api/complete-task-path.v1.valid.json",
        "schemas/fixtures/api/complete-task-path.v1.invalid.json",
        CompleteTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:complete-task-response:v1",
        "api/complete-task-response.v1.schema.json",
        "Kanban complete task response v1",
        "api.complete-task.response",
        "schemas/fixtures/api/complete-task-response.v1.valid.json",
        "schemas/fixtures/api/complete-task-response.v1.invalid.json",
        CompleteTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:submit-review-task-request:v1",
        "api/submit-review-task-request.v1.schema.json",
        "Kanban submit review task request v1",
        "api.submit-review-task.request",
        "schemas/fixtures/api/submit-review-task-request.v1.valid.json",
        "schemas/fixtures/api/submit-review-task-request.v1.invalid.json",
        SubmitReviewTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:submit-review-task-path:v1",
        "api/submit-review-task-path.v1.schema.json",
        "Kanban submit review task path v1",
        "api.submit-review-task.path",
        "schemas/fixtures/api/submit-review-task-path.v1.valid.json",
        "schemas/fixtures/api/submit-review-task-path.v1.invalid.json",
        SubmitReviewTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:submit-review-task-response:v1",
        "api/submit-review-task-response.v1.schema.json",
        "Kanban submit review task response v1",
        "api.submit-review-task.response",
        "schemas/fixtures/api/submit-review-task-response.v1.valid.json",
        "schemas/fixtures/api/submit-review-task-response.v1.invalid.json",
        SubmitReviewTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:block-task-request:v1",
        "api/block-task-request.v1.schema.json",
        "Kanban block task request v1",
        "api.block-task.request",
        "schemas/fixtures/api/block-task-request.v1.valid.json",
        "schemas/fixtures/api/block-task-request.v1.invalid.json",
        BlockTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:block-task-path:v1",
        "api/block-task-path.v1.schema.json",
        "Kanban block task path v1",
        "api.block-task.path",
        "schemas/fixtures/api/block-task-path.v1.valid.json",
        "schemas/fixtures/api/block-task-path.v1.invalid.json",
        BlockTaskPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:block-task-response:v1",
        "api/block-task-response.v1.schema.json",
        "Kanban block task response v1",
        "api.block-task.response",
        "schemas/fixtures/api/block-task-response.v1.valid.json",
        "schemas/fixtures/api/block-task-response.v1.invalid.json",
        BlockTaskResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:unblock-task-request:v1",
        "api/unblock-task-request.v1.schema.json",
        "Kanban unblock task request v1",
        "api.unblock-task.request",
        "schemas/fixtures/api/unblock-task-request.v1.valid.json",
        "schemas/fixtures/api/unblock-task-request.v1.invalid.json",
        UnblockTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reopen-task-request:v1",
        "api/reopen-task-request.v1.schema.json",
        "Kanban reopen task request v1",
        "api.reopen-task.request",
        "schemas/fixtures/api/reopen-task-request.v1.valid.json",
        "schemas/fixtures/api/reopen-task-request.v1.invalid.json",
        ReopenTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:archive-task-request:v1",
        "api/archive-task-request.v1.schema.json",
        "Kanban archive task request v1",
        "api.archive-task.request",
        "schemas/fixtures/api/archive-task-request.v1.valid.json",
        "schemas/fixtures/api/archive-task-request.v1.invalid.json",
        ArchiveTaskRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:add-dependency-request:v1",
        "api/add-dependency-request.v1.schema.json",
        "Kanban add dependency request v1",
        "api.add-dependency.request",
        "schemas/fixtures/api/add-dependency-request.v1.valid.json",
        "schemas/fixtures/api/add-dependency-request.v1.invalid.json",
        AddDependencyRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-dependencies-path:v1",
        "api/list-dependencies-path.v1.schema.json",
        "Kanban API list dependencies path v1",
        "api.list-dependencies.path",
        "schemas/fixtures/api/list-dependencies-path.v1.valid.json",
        "schemas/fixtures/api/list-dependencies-path.v1.invalid.json",
        ListDependenciesPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-dependencies-response:v1",
        "api/list-dependencies-response.v1.schema.json",
        "Kanban API list dependencies response v1",
        "api.list-dependencies.response",
        "schemas/fixtures/api/list-dependencies-response.v1.valid.json",
        "schemas/fixtures/api/list-dependencies-response.v1.invalid.json",
        ListDependenciesResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:add-dependency-path:v1",
        "api/add-dependency-path.v1.schema.json",
        "Kanban API add dependency path v1",
        "api.add-dependency.path",
        "schemas/fixtures/api/add-dependency-path.v1.valid.json",
        "schemas/fixtures/api/add-dependency-path.v1.invalid.json",
        AddDependencyPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:add-dependency-response:v1",
        "api/add-dependency-response.v1.schema.json",
        "Kanban API add dependency response v1",
        "api.add-dependency.response",
        "schemas/fixtures/api/add-dependency-response.v1.valid.json",
        "schemas/fixtures/api/add-dependency-response.v1.invalid.json",
        AddDependencyResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:remove-dependency-path:v1",
        "api/remove-dependency-path.v1.schema.json",
        "Kanban API remove dependency path v1",
        "api.remove-dependency.path",
        "schemas/fixtures/api/remove-dependency-path.v1.valid.json",
        "schemas/fixtures/api/remove-dependency-path.v1.invalid.json",
        RemoveDependencyPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:remove-dependency-response:v1",
        "api/remove-dependency-response.v1.schema.json",
        "Kanban API remove dependency response v1",
        "api.remove-dependency.response",
        "schemas/fixtures/api/remove-dependency-response.v1.valid.json",
        "schemas/fixtures/api/remove-dependency-response.v1.invalid.json",
        RemoveDependencyResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-path:v1",
        "api/mark-execution-plan-not-required-path.v1.schema.json",
        "Kanban API mark execution plan not required path v1",
        "api.mark-execution-plan-not-required.path",
        "schemas/fixtures/api/mark-execution-plan-not-required-path.v1.valid.json",
        "schemas/fixtures/api/mark-execution-plan-not-required-path.v1.invalid.json",
        MarkExecutionPlanNotRequiredPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-request:v1",
        "api/mark-execution-plan-not-required-request.v1.schema.json",
        "Kanban API mark execution plan not required request v1",
        "api.mark-execution-plan-not-required.request",
        "schemas/fixtures/api/mark-execution-plan-not-required-request.v1.valid.json",
        "schemas/fixtures/api/mark-execution-plan-not-required-request.v1.invalid.json",
        MarkExecutionPlanNotRequiredRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-response:v1",
        "api/mark-execution-plan-not-required-response.v1.schema.json",
        "Kanban API mark execution plan not required response v1",
        "api.mark-execution-plan-not-required.response",
        "schemas/fixtures/api/mark-execution-plan-not-required-response.v1.valid.json",
        "schemas/fixtures/api/mark-execution-plan-not-required-response.v1.invalid.json",
        MarkExecutionPlanNotRequiredResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-run-log-path:v1",
        "api/get-run-log-path.v1.schema.json",
        "Kanban API get run log path v1",
        "api.get-run-log.path",
        "schemas/fixtures/api/get-run-log-path.v1.valid.json",
        "schemas/fixtures/api/get-run-log-path.v1.invalid.json",
        GetRunLogPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-run-log-response:v1",
        "api/get-run-log-response.v1.schema.json",
        "Kanban API get run log response v1",
        "api.get-run-log.response",
        "schemas/fixtures/api/get-run-log-response.v1.valid.json",
        "schemas/fixtures/api/get-run-log-response.v1.invalid.json",
        GetRunLogResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-attachments-path:v1",
        "api/list-attachments-path.v1.schema.json",
        "Kanban API list attachments path v1",
        "api.list-attachments.path",
        "schemas/fixtures/api/list-attachments-path.v1.valid.json",
        "schemas/fixtures/api/list-attachments-path.v1.invalid.json",
        ListAttachmentsPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-attachments-response:v1",
        "api/list-attachments-response.v1.schema.json",
        "Kanban API list attachments response v1",
        "api.list-attachments.response",
        "schemas/fixtures/api/list-attachments-response.v1.valid.json",
        "schemas/fixtures/api/list-attachments-response.v1.invalid.json",
        ListAttachmentsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-attachment-path:v1",
        "api/create-attachment-path.v1.schema.json",
        "Kanban API create attachment path v1",
        "api.create-attachment.path",
        "schemas/fixtures/api/create-attachment-path.v1.valid.json",
        "schemas/fixtures/api/create-attachment-path.v1.invalid.json",
        CreateAttachmentPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-attachment-request:v1",
        "api/create-attachment-request.v1.schema.json",
        "Kanban API create attachment request v1",
        "api.create-attachment.request",
        "schemas/fixtures/api/create-attachment-request.v1.valid.json",
        "schemas/fixtures/api/create-attachment-request.v1.invalid.json",
        CreateAttachmentRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:create-attachment-response:v1",
        "api/create-attachment-response.v1.schema.json",
        "Kanban API create attachment response v1",
        "api.create-attachment.response",
        "schemas/fixtures/api/create-attachment-response.v1.valid.json",
        "schemas/fixtures/api/create-attachment-response.v1.invalid.json",
        CreateAttachmentResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:download-attachment-path:v1",
        "api/download-attachment-path.v1.schema.json",
        "Kanban API download attachment path v1",
        "api.download-attachment.path",
        "schemas/fixtures/api/download-attachment-path.v1.valid.json",
        "schemas/fixtures/api/download-attachment-path.v1.invalid.json",
        crate::GetAttachmentPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:download-attachment-response:v1",
        "api/download-attachment-response.v1.schema.json",
        "Kanban API download attachment bytes v1",
        "api.download-attachment.response",
        "schemas/fixtures/api/download-attachment-response.v1.valid.json",
        "schemas/fixtures/api/download-attachment-response.v1.invalid.json",
        AttachmentDownloadResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:delete-attachment-path:v1",
        "api/delete-attachment-path.v1.schema.json",
        "Kanban API delete attachment path v1",
        "api.delete-attachment.path",
        "schemas/fixtures/api/delete-attachment-path.v1.valid.json",
        "schemas/fixtures/api/delete-attachment-path.v1.invalid.json",
        DeleteAttachmentPath
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:delete-attachment-response:v1",
        "api/delete-attachment-response.v1.schema.json",
        "Kanban API delete attachment response v1",
        "api.delete-attachment.response",
        "schemas/fixtures/api/delete-attachment-response.v1.valid.json",
        "schemas/fixtures/api/delete-attachment-response.v1.invalid.json",
        DeleteAttachmentResponse
    ),
    cli_response_schema_root!("attachment-add", "attachment add", CliAttachmentAddOutput),
    cli_response_schema_root!(
        "attachment-list",
        "attachment list",
        CliAttachmentListOutput
    ),
    cli_response_schema_root!(
        "attachment-remove",
        "attachment remove",
        CliAttachmentRemoveOutput
    ),
    SchemaRoot {
        id: "urn:kanban-tool:schema:metadata:decision:v1",
        artifact_path: "metadata/decision.v1.schema.json",
        title: "Kanban decision metadata v1",
        contract_id: "metadata.decision.input",
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::Typed,
        valid_fixture: "schemas/fixtures/metadata/decision.v1.valid.json",
        invalid_fixture: "schemas/fixtures/metadata/decision.v1.invalid.json",
        generate: generate_for::<DecisionMetadata>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:metadata:signal-record-input:v1",
        artifact_path: "metadata/signal-record-input.v1.schema.json",
        title: "Kanban signal record metadata input v1",
        contract_id: "metadata.signal-record.input",
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::Typed,
        valid_fixture: "schemas/fixtures/metadata/signal-record-input.v1.valid.json",
        invalid_fixture: "schemas/fixtures/metadata/signal-record-input.v1.invalid.json",
        generate: generate_for::<crate::structured_metadata::SignalRecordMetadataInput>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:metadata:signal-link-output:v1",
        artifact_path: "metadata/signal-link-output.v1.schema.json",
        title: "Kanban signal backlink metadata output v1",
        contract_id: "metadata.signal-link.output",
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::Typed,
        valid_fixture: "schemas/fixtures/metadata/signal-link-output.v1.valid.json",
        invalid_fixture: "schemas/fixtures/metadata/signal-link-output.v1.invalid.json",
        generate: generate_for::<crate::structured_metadata::SignalLinkMetadataOutput>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:metadata:label-proposal-candidate-input:v1",
        artifact_path: "metadata/label-proposal-candidate-input.v1.schema.json",
        title: "Kanban label proposal candidate metadata input v1",
        contract_id: "metadata.label-proposal-candidate.input",
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::Typed,
        valid_fixture: "schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json",
        invalid_fixture: "schemas/fixtures/metadata/label-proposal-candidate-input.v1.invalid.json",
        generate: generate_for::<crate::structured_metadata::LabelProposalCandidateMetadataInput>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:metadata:ontology-record-input:v1",
        artifact_path: "metadata/ontology-record-input.v1.schema.json",
        title: "Kanban label ontology record metadata input v1",
        contract_id: "metadata.ontology-record.input",
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::Typed,
        valid_fixture: "schemas/fixtures/metadata/ontology-record-input.v1.valid.json",
        invalid_fixture: "schemas/fixtures/metadata/ontology-record-input.v1.invalid.json",
        generate: generate_for::<crate::structured_metadata::OntologyRecordMetadataInput>,
    },
    SchemaRoot {
        id: "urn:kanban-tool:schema:metadata:ontology-validation-evidence-input:v1",
        artifact_path: "metadata/ontology-validation-evidence-input.v1.schema.json",
        title: "Kanban label ontology validation evidence metadata input v1",
        contract_id: "metadata.ontology-validation-evidence.input",
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::OpaqueExtension,
        valid_fixture: "schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json",
        invalid_fixture: "schemas/fixtures/metadata/ontology-validation-evidence-input.v1.invalid.json",
        generate: generate_for::<crate::structured_metadata::OntologyValidationEvidenceMetadataInput>,
    },
    request_schema_root!(
        "urn:kanban-tool:schema:api:label-atom-path:v1",
        "api/label-atom-path.v1.schema.json",
        "Label atom path v1",
        "api.label-atom.path",
        "schemas/fixtures/api/label-atom-path.v1.valid.json",
        "schemas/fixtures/api/label-atom-path.v1.invalid.json",
        LabelAtomPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:label-suggestion-query:v1",
        "api/label-suggestion-query.v1.schema.json",
        "Label suggestion query v1",
        "api.label-suggestion.query",
        "schemas/fixtures/api/label-suggestion-query.v1.valid.json",
        "schemas/fixtures/api/label-suggestion-query.v1.invalid.json",
        LabelSuggestionQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:propose-task-label-query:v1",
        "api/propose-task-label-query.v1.schema.json",
        "Propose task label query v1",
        "api.propose-task-label.query",
        "schemas/fixtures/api/propose-task-label-query.v1.valid.json",
        "schemas/fixtures/api/propose-task-label-query.v1.invalid.json",
        LabelSuggestionQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:label-ontology-review-query:v1",
        "api/label-ontology-review-query.v1.schema.json",
        "Label ontology review query v1",
        "api.label-ontology-review.query",
        "schemas/fixtures/api/label-ontology-review-query.v1.valid.json",
        "schemas/fixtures/api/label-ontology-review-query.v1.invalid.json",
        LabelOntologyReviewQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:label-ontology-signal-query:v1",
        "api/label-ontology-signal-query.v1.schema.json",
        "Label ontology signal query v1",
        "api.label-ontology-signal.query",
        "schemas/fixtures/api/label-ontology-signal-query.v1.valid.json",
        "schemas/fixtures/api/label-ontology-signal-query.v1.invalid.json",
        LabelOntologySignalQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:delete-label-semantics-query:v1",
        "api/delete-label-semantics-query.v1.schema.json",
        "Delete label semantics query v1",
        "api.delete-label-semantics.query",
        "schemas/fixtures/api/delete-label-semantics-query.v1.valid.json",
        "schemas/fixtures/api/delete-label-semantics-query.v1.invalid.json",
        DeleteLabelSemanticsQuery
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-board-labels-response:v1",
        "api/list-board-labels-response.v1.schema.json",
        "List board labels response v1",
        "api.list-board-labels.response",
        "schemas/fixtures/api/list-board-labels-response.v1.valid.json",
        "schemas/fixtures/api/list-board-labels-response.v1.invalid.json",
        ListBoardLabelsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-board-label-request:v1",
        "api/create-board-label-request.v1.schema.json",
        "Create board label request v1",
        "api.create-board-label.request",
        "schemas/fixtures/api/create-board-label-request.v1.valid.json",
        "schemas/fixtures/api/create-board-label-request.v1.invalid.json",
        CreateBoardLabelRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:create-board-label-response:v1",
        "api/create-board-label-response.v1.schema.json",
        "Create board label response v1",
        "api.create-board-label.response",
        "schemas/fixtures/api/create-board-label-response.v1.valid.json",
        "schemas/fixtures/api/create-board-label-response.v1.invalid.json",
        CreateBoardLabelResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-label-semantics-response:v1",
        "api/list-label-semantics-response.v1.schema.json",
        "List label semantics response v1",
        "api.list-label-semantics.response",
        "schemas/fixtures/api/list-label-semantics-response.v1.valid.json",
        "schemas/fixtures/api/list-label-semantics-response.v1.invalid.json",
        ListLabelSemanticsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-label-semantics-response:v1",
        "api/get-label-semantics-response.v1.schema.json",
        "Get label semantics response v1",
        "api.get-label-semantics.response",
        "schemas/fixtures/api/get-label-semantics-response.v1.valid.json",
        "schemas/fixtures/api/get-label-semantics-response.v1.invalid.json",
        GetLabelSemanticsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:upsert-label-semantics-request:v1",
        "api/upsert-label-semantics-request.v1.schema.json",
        "Upsert label semantics request v1",
        "api.upsert-label-semantics.request",
        "schemas/fixtures/api/upsert-label-semantics-request.v1.valid.json",
        "schemas/fixtures/api/upsert-label-semantics-request.v1.invalid.json",
        UpsertLabelSemanticsRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:upsert-label-semantics-response:v1",
        "api/upsert-label-semantics-response.v1.schema.json",
        "Upsert label semantics response v1",
        "api.upsert-label-semantics.response",
        "schemas/fixtures/api/upsert-label-semantics-response.v1.valid.json",
        "schemas/fixtures/api/upsert-label-semantics-response.v1.invalid.json",
        UpsertLabelSemanticsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-label-atoms-response:v1",
        "api/list-label-atoms-response.v1.schema.json",
        "List label atoms response v1",
        "api.list-label-atoms.response",
        "schemas/fixtures/api/list-label-atoms-response.v1.valid.json",
        "schemas/fixtures/api/list-label-atoms-response.v1.invalid.json",
        ListLabelAtomsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:explain-label-atom-response:v1",
        "api/explain-label-atom-response.v1.schema.json",
        "Explain label atom response v1",
        "api.explain-label-atom.response",
        "schemas/fixtures/api/explain-label-atom-response.v1.valid.json",
        "schemas/fixtures/api/explain-label-atom-response.v1.invalid.json",
        ExplainLabelAtomResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:label-atom-index-status-response:v1",
        "api/label-atom-index-status-response.v1.schema.json",
        "Label atom index status response v1",
        "api.label-atom-index-status.response",
        "schemas/fixtures/api/label-atom-index-status-response.v1.valid.json",
        "schemas/fixtures/api/label-atom-index-status-response.v1.invalid.json",
        LabelAtomIndexStatusResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:rebuild-label-atom-index-response:v1",
        "api/rebuild-label-atom-index-response.v1.schema.json",
        "Rebuild label atom index response v1",
        "api.rebuild-label-atom-index.response",
        "schemas/fixtures/api/rebuild-label-atom-index-response.v1.valid.json",
        "schemas/fixtures/api/rebuild-label-atom-index-response.v1.invalid.json",
        RebuildLabelAtomIndexResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:query-label-atom-index-response:v1",
        "api/query-label-atom-index-response.v1.schema.json",
        "Query label atom index response v1",
        "api.query-label-atom-index.response",
        "schemas/fixtures/api/query-label-atom-index-response.v1.valid.json",
        "schemas/fixtures/api/query-label-atom-index-response.v1.invalid.json",
        QueryLabelAtomIndexResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-signals-response:v1",
        "api/list-signals-response.v1.schema.json",
        "List signals response v1",
        "api.list-signals.response",
        "schemas/fixtures/api/list-signals-response.v1.valid.json",
        "schemas/fixtures/api/list-signals-response.v1.invalid.json",
        ListSignalsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:review-signals-response:v1",
        "api/review-signals-response.v1.schema.json",
        "Review signals response v1",
        "api.review-signals.response",
        "schemas/fixtures/api/review-signals-response.v1.valid.json",
        "schemas/fixtures/api/review-signals-response.v1.invalid.json",
        ReviewSignalsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-signal-response:v1",
        "api/get-signal-response.v1.schema.json",
        "Get signal response v1",
        "api.get-signal.response",
        "schemas/fixtures/api/get-signal-response.v1.valid.json",
        "schemas/fixtures/api/get-signal-response.v1.invalid.json",
        GetSignalResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:record-signal-request:v1",
        "api/record-signal-request.v1.schema.json",
        "Record signal request v1",
        "api.record-signal.request",
        "schemas/fixtures/api/record-signal-request.v1.valid.json",
        "schemas/fixtures/api/record-signal-request.v1.invalid.json",
        RecordSignalRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:record-signal-response:v1",
        "api/record-signal-response.v1.schema.json",
        "Record signal response v1",
        "api.record-signal.response",
        "schemas/fixtures/api/record-signal-response.v1.valid.json",
        "schemas/fixtures/api/record-signal-response.v1.invalid.json",
        RecordSignalResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:review-signals-request:v1",
        "api/review-signals-request.v1.schema.json",
        "Review signals request v1",
        "api.confirm-signals.request",
        "schemas/fixtures/api/review-signals-request.v1.valid.json",
        "schemas/fixtures/api/review-signals-request.v1.invalid.json",
        ReviewSignalsRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reject-signals-request:v1",
        "api/reject-signals-request.v1.schema.json",
        "Reject signals request v1",
        "api.reject-signals.request",
        "schemas/fixtures/api/reject-signals-request.v1.valid.json",
        "schemas/fixtures/api/reject-signals-request.v1.invalid.json",
        ReviewSignalsRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:resolve-signals-request:v1",
        "api/resolve-signals-request.v1.schema.json",
        "Resolve signals request v1",
        "api.resolve-signals.request",
        "schemas/fixtures/api/resolve-signals-request.v1.valid.json",
        "schemas/fixtures/api/resolve-signals-request.v1.invalid.json",
        ReviewSignalsRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:supersede-signals-request:v1",
        "api/supersede-signals-request.v1.schema.json",
        "Supersede signals request v1",
        "api.supersede-signals.request",
        "schemas/fixtures/api/supersede-signals-request.v1.valid.json",
        "schemas/fixtures/api/supersede-signals-request.v1.invalid.json",
        ReviewSignalsRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:confirm-signals-response:v1",
        "api/confirm-signals-response.v1.schema.json",
        "Confirm signals response v1",
        "api.confirm-signals.response",
        "schemas/fixtures/api/confirm-signals-response.v1.valid.json",
        "schemas/fixtures/api/confirm-signals-response.v1.invalid.json",
        ConfirmSignalsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:reject-signals-response:v1",
        "api/reject-signals-response.v1.schema.json",
        "Reject signals response v1",
        "api.reject-signals.response",
        "schemas/fixtures/api/reject-signals-response.v1.valid.json",
        "schemas/fixtures/api/reject-signals-response.v1.invalid.json",
        RejectSignalsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:resolve-signals-response:v1",
        "api/resolve-signals-response.v1.schema.json",
        "Resolve signals response v1",
        "api.resolve-signals.response",
        "schemas/fixtures/api/resolve-signals-response.v1.valid.json",
        "schemas/fixtures/api/resolve-signals-response.v1.invalid.json",
        ResolveSignalsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:supersede-signals-response:v1",
        "api/supersede-signals-response.v1.schema.json",
        "Supersede signals response v1",
        "api.supersede-signals.response",
        "schemas/fixtures/api/supersede-signals-response.v1.valid.json",
        "schemas/fixtures/api/supersede-signals-response.v1.invalid.json",
        SupersedeSignalsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:suggest-task-labels-response:v1",
        "api/suggest-task-labels-response.v1.schema.json",
        "Suggest task labels response v1",
        "api.suggest-task-labels.response",
        "schemas/fixtures/api/suggest-task-labels-response.v1.valid.json",
        "schemas/fixtures/api/suggest-task-labels-response.v1.invalid.json",
        SuggestTaskLabelsResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:list-task-label-proposals-response:v1",
        "api/list-task-label-proposals-response.v1.schema.json",
        "List task label proposals response v1",
        "api.list-task-label-proposals.response",
        "schemas/fixtures/api/list-task-label-proposals-response.v1.valid.json",
        "schemas/fixtures/api/list-task-label-proposals-response.v1.invalid.json",
        ListTaskLabelProposalsResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:propose-task-label-request:v1",
        "api/propose-task-label-request.v1.schema.json",
        "Propose task label request v1",
        "api.propose-task-label.request",
        "schemas/fixtures/api/propose-task-label-request.v1.valid.json",
        "schemas/fixtures/api/propose-task-label-request.v1.invalid.json",
        ProposeTaskLabelRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:propose-task-label-response:v1",
        "api/propose-task-label-response.v1.schema.json",
        "Propose task label response v1",
        "api.propose-task-label.response",
        "schemas/fixtures/api/propose-task-label-response.v1.valid.json",
        "schemas/fixtures/api/propose-task-label-response.v1.invalid.json",
        ProposeTaskLabelResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:record-label-ontology-observation-body:v1",
        "api/record-label-ontology-observation-body.v1.schema.json",
        "Record label ontology observation request v1",
        "api.record-label-ontology-observation.body",
        "schemas/fixtures/api/record-label-ontology-observation-body.v1.valid.json",
        "schemas/fixtures/api/record-label-ontology-observation-body.v1.invalid.json",
        RecordLabelOntologyObservationRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:record-label-ontology-observation-response:v1",
        "api/record-label-ontology-observation-response.v1.schema.json",
        "Record label ontology observation response v1",
        "api.record-label-ontology-observation.response",
        "schemas/fixtures/api/record-label-ontology-observation-response.v1.valid.json",
        "schemas/fixtures/api/record-label-ontology-observation-response.v1.invalid.json",
        RecordLabelOntologyObservationResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:review-label-ontology-response:v1",
        "api/review-label-ontology-response.v1.schema.json",
        "Review label ontology response v1",
        "api.review-label-ontology.response",
        "schemas/fixtures/api/review-label-ontology-response.v1.valid.json",
        "schemas/fixtures/api/review-label-ontology-response.v1.invalid.json",
        ReviewLabelOntologyResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-label-ontology-action-request:v1",
        "api/create-label-ontology-action-request.v1.schema.json",
        "Create label ontology action request v1",
        "api.create-label-ontology-action.request",
        "schemas/fixtures/api/create-label-ontology-action-request.v1.valid.json",
        "schemas/fixtures/api/create-label-ontology-action-request.v1.invalid.json",
        LabelOntologyActionRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:create-label-ontology-action-response:v1",
        "api/create-label-ontology-action-response.v1.schema.json",
        "Create label ontology action response v1",
        "api.create-label-ontology-action.response",
        "schemas/fixtures/api/create-label-ontology-action-response.v1.valid.json",
        "schemas/fixtures/api/create-label-ontology-action-response.v1.invalid.json",
        LabelOntologyActionResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-request:v1",
        "api/apply-label-ontology-atom-request.v1.schema.json",
        "Apply label ontology atom request v1",
        "api.apply-label-ontology-atom.request",
        "schemas/fixtures/api/apply-label-ontology-atom-request.v1.valid.json",
        "schemas/fixtures/api/apply-label-ontology-atom-request.v1.invalid.json",
        ApplyLabelOntologyAtomRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-response:v1",
        "api/apply-label-ontology-atom-response.v1.schema.json",
        "Apply label ontology atom response v1",
        "api.apply-label-ontology-atom.response",
        "schemas/fixtures/api/apply-label-ontology-atom-response.v1.valid.json",
        "schemas/fixtures/api/apply-label-ontology-atom-response.v1.invalid.json",
        LabelOntologyActionResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-request:v1",
        "api/revert-label-ontology-mutation-request.v1.schema.json",
        "Revert label ontology mutation request v1",
        "api.revert-label-ontology-mutation.request",
        "schemas/fixtures/api/revert-label-ontology-mutation-request.v1.valid.json",
        "schemas/fixtures/api/revert-label-ontology-mutation-request.v1.invalid.json",
        RevertLabelOntologyMutationRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-response:v1",
        "api/revert-label-ontology-mutation-response.v1.schema.json",
        "Revert label ontology mutation response v1",
        "api.revert-label-ontology-mutation.response",
        "schemas/fixtures/api/revert-label-ontology-mutation-response.v1.valid.json",
        "schemas/fixtures/api/revert-label-ontology-mutation-response.v1.invalid.json",
        LabelOntologyActionResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:validate-label-ontology-action-request:v1",
        "api/validate-label-ontology-action-request.v1.schema.json",
        "Validate label ontology action request v1",
        "api.validate-label-ontology-action.request",
        "schemas/fixtures/api/validate-label-ontology-action-request.v1.valid.json",
        "schemas/fixtures/api/validate-label-ontology-action-request.v1.invalid.json",
        ValidateLabelOntologyActionRequest
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:validate-label-ontology-action-response:v1",
        "api/validate-label-ontology-action-response.v1.schema.json",
        "Validate label ontology action response v1",
        "api.validate-label-ontology-action.response",
        "schemas/fixtures/api/validate-label-ontology-action-response.v1.valid.json",
        "schemas/fixtures/api/validate-label-ontology-action-response.v1.invalid.json",
        LabelOntologyActionResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-label-ontology-signal-response:v1",
        "api/get-label-ontology-signal-response.v1.schema.json",
        "Get label ontology signal response v1",
        "api.get-label-ontology-signal.response",
        "schemas/fixtures/api/get-label-ontology-signal-response.v1.valid.json",
        "schemas/fixtures/api/get-label-ontology-signal-response.v1.invalid.json",
        GetLabelOntologySignalResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:get-label-proposal-response:v1",
        "api/get-label-proposal-response.v1.schema.json",
        "Get label proposal response v1",
        "api.get-label-proposal.response",
        "schemas/fixtures/api/get-label-proposal-response.v1.valid.json",
        "schemas/fixtures/api/get-label-proposal-response.v1.invalid.json",
        GetLabelProposalResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:accept-label-proposal-response:v1",
        "api/accept-label-proposal-response.v1.schema.json",
        "Accept label proposal response v1",
        "api.accept-label-proposal.response",
        "schemas/fixtures/api/accept-label-proposal-response.v1.valid.json",
        "schemas/fixtures/api/accept-label-proposal-response.v1.invalid.json",
        LabelProposalDecisionResponse
    ),
    response_schema_root!(
        "urn:kanban-tool:schema:api:reject-label-proposal-response:v1",
        "api/reject-label-proposal-response.v1.schema.json",
        "Reject label proposal response v1",
        "api.reject-label-proposal.response",
        "schemas/fixtures/api/reject-label-proposal-response.v1.valid.json",
        "schemas/fixtures/api/reject-label-proposal-response.v1.invalid.json",
        LabelProposalDecisionResponse
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-task-label-proposals-path:v1",
        "api/list-task-label-proposals-path.v1.schema.json",
        "List Task Label Proposals Path v1",
        "api.list-task-label-proposals.path",
        "schemas/fixtures/api/list-task-label-proposals-path.v1.valid.json",
        "schemas/fixtures/api/list-task-label-proposals-path.v1.invalid.json",
        TaskLabelSurfacePath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-label-ontology-signals-path:v1",
        "api/list-label-ontology-signals-path.v1.schema.json",
        "List Label Ontology Signals Path v1",
        "api.list-label-ontology-signals.path",
        "schemas/fixtures/api/list-label-ontology-signals-path.v1.valid.json",
        "schemas/fixtures/api/list-label-ontology-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-label-semantics-path:v1",
        "api/get-label-semantics-path.v1.schema.json",
        "Get Label Semantics Path v1",
        "api.get-label-semantics.path",
        "schemas/fixtures/api/get-label-semantics-path.v1.valid.json",
        "schemas/fixtures/api/get-label-semantics-path.v1.invalid.json",
        LabelSemanticsPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:review-signals-path:v1",
        "api/review-signals-path.v1.schema.json",
        "Review Signals Path v1",
        "api.review-signals.path",
        "schemas/fixtures/api/review-signals-path.v1.valid.json",
        "schemas/fixtures/api/review-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-label-atoms-path:v1",
        "api/list-label-atoms-path.v1.schema.json",
        "List Label Atoms Path v1",
        "api.list-label-atoms.path",
        "schemas/fixtures/api/list-label-atoms-path.v1.valid.json",
        "schemas/fixtures/api/list-label-atoms-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:rebuild-label-atom-index-path:v1",
        "api/rebuild-label-atom-index-path.v1.schema.json",
        "Rebuild Label Atom Index Path v1",
        "api.rebuild-label-atom-index.path",
        "schemas/fixtures/api/rebuild-label-atom-index-path.v1.valid.json",
        "schemas/fixtures/api/rebuild-label-atom-index-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-label-ontology-action-path:v1",
        "api/create-label-ontology-action-path.v1.schema.json",
        "Create Label Ontology Action Path v1",
        "api.create-label-ontology-action.path",
        "schemas/fixtures/api/create-label-ontology-action-path.v1.valid.json",
        "schemas/fixtures/api/create-label-ontology-action-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-signals-query:v1",
        "api/list-signals-query.v1.schema.json",
        "List Signals Query v1",
        "api.list-signals.query",
        "schemas/fixtures/api/list-signals-query.v1.valid.json",
        "schemas/fixtures/api/list-signals-query.v1.invalid.json",
        SignalQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:accept-label-proposal-body:v1",
        "api/accept-label-proposal-body.v1.schema.json",
        "Accept Label Proposal Body v1",
        "api.accept-label-proposal.body",
        "schemas/fixtures/api/accept-label-proposal-body.v1.valid.json",
        "schemas/fixtures/api/accept-label-proposal-body.v1.invalid.json",
        LabelProposalDecisionRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:review-label-ontology-path:v1",
        "api/review-label-ontology-path.v1.schema.json",
        "Review Label Ontology Path v1",
        "api.review-label-ontology.path",
        "schemas/fixtures/api/review-label-ontology-path.v1.valid.json",
        "schemas/fixtures/api/review-label-ontology-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:suggest-task-labels-path:v1",
        "api/suggest-task-labels-path.v1.schema.json",
        "Suggest Task Labels Path v1",
        "api.suggest-task-labels.path",
        "schemas/fixtures/api/suggest-task-labels-path.v1.valid.json",
        "schemas/fixtures/api/suggest-task-labels-path.v1.invalid.json",
        TaskLabelSurfacePath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:query-label-atom-index-path:v1",
        "api/query-label-atom-index-path.v1.schema.json",
        "Query Label Atom Index Path v1",
        "api.query-label-atom-index.path",
        "schemas/fixtures/api/query-label-atom-index-path.v1.valid.json",
        "schemas/fixtures/api/query-label-atom-index-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:label-atom-index-status-path:v1",
        "api/label-atom-index-status-path.v1.schema.json",
        "Label Atom Index Status Path v1",
        "api.label-atom-index-status.path",
        "schemas/fixtures/api/label-atom-index-status-path.v1.valid.json",
        "schemas/fixtures/api/label-atom-index-status-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-label-proposal-path:v1",
        "api/get-label-proposal-path.v1.schema.json",
        "Get Label Proposal Path v1",
        "api.get-label-proposal.path",
        "schemas/fixtures/api/get-label-proposal-path.v1.valid.json",
        "schemas/fixtures/api/get-label-proposal-path.v1.invalid.json",
        ProposalPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-label-semantics-path:v1",
        "api/list-label-semantics-path.v1.schema.json",
        "List Label Semantics Path v1",
        "api.list-label-semantics.path",
        "schemas/fixtures/api/list-label-semantics-path.v1.valid.json",
        "schemas/fixtures/api/list-label-semantics-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reject-label-proposal-body:v1",
        "api/reject-label-proposal-body.v1.schema.json",
        "Reject Label Proposal Body v1",
        "api.reject-label-proposal.body",
        "schemas/fixtures/api/reject-label-proposal-body.v1.valid.json",
        "schemas/fixtures/api/reject-label-proposal-body.v1.invalid.json",
        LabelProposalDecisionRequest
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:accept-label-proposal-path:v1",
        "api/accept-label-proposal-path.v1.schema.json",
        "Accept Label Proposal Path v1",
        "api.accept-label-proposal.path",
        "schemas/fixtures/api/accept-label-proposal-path.v1.valid.json",
        "schemas/fixtures/api/accept-label-proposal-path.v1.invalid.json",
        ProposalPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-path:v1",
        "api/apply-label-ontology-atom-path.v1.schema.json",
        "Apply Label Ontology Atom Path v1",
        "api.apply-label-ontology-atom.path",
        "schemas/fixtures/api/apply-label-ontology-atom-path.v1.valid.json",
        "schemas/fixtures/api/apply-label-ontology-atom-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:record-label-ontology-observation-path:v1",
        "api/record-label-ontology-observation-path.v1.schema.json",
        "Record Label Ontology Observation Path v1",
        "api.record-label-ontology-observation.path",
        "schemas/fixtures/api/record-label-ontology-observation-path.v1.valid.json",
        "schemas/fixtures/api/record-label-ontology-observation-path.v1.invalid.json",
        TaskLabelSurfacePath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-label-ontology-signal-path:v1",
        "api/get-label-ontology-signal-path.v1.schema.json",
        "Get Label Ontology Signal Path v1",
        "api.get-label-ontology-signal.path",
        "schemas/fixtures/api/get-label-ontology-signal-path.v1.valid.json",
        "schemas/fixtures/api/get-label-ontology-signal-path.v1.invalid.json",
        SignalPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:review-signals-query:v1",
        "api/review-signals-query.v1.schema.json",
        "Review Signals Query v1",
        "api.review-signals.query",
        "schemas/fixtures/api/review-signals-query.v1.valid.json",
        "schemas/fixtures/api/review-signals-query.v1.invalid.json",
        SignalQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-board-labels-path:v1",
        "api/list-board-labels-path.v1.schema.json",
        "List Board Labels Path v1",
        "api.list-board-labels.path",
        "schemas/fixtures/api/list-board-labels-path.v1.valid.json",
        "schemas/fixtures/api/list-board-labels-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reject-label-proposal-path:v1",
        "api/reject-label-proposal-path.v1.schema.json",
        "Reject Label Proposal Path v1",
        "api.reject-label-proposal.path",
        "schemas/fixtures/api/reject-label-proposal-path.v1.valid.json",
        "schemas/fixtures/api/reject-label-proposal-path.v1.invalid.json",
        ProposalPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:list-signals-path:v1",
        "api/list-signals-path.v1.schema.json",
        "List Signals Path v1",
        "api.list-signals.path",
        "schemas/fixtures/api/list-signals-path.v1.valid.json",
        "schemas/fixtures/api/list-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:create-board-label-path:v1",
        "api/create-board-label-path.v1.schema.json",
        "Create Board Label Path v1",
        "api.create-board-label.path",
        "schemas/fixtures/api/create-board-label-path.v1.valid.json",
        "schemas/fixtures/api/create-board-label-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:query-label-atom-index-query:v1",
        "api/query-label-atom-index-query.v1.schema.json",
        "Query Label Atom Index Query v1",
        "api.query-label-atom-index.query",
        "schemas/fixtures/api/query-label-atom-index-query.v1.valid.json",
        "schemas/fixtures/api/query-label-atom-index-query.v1.invalid.json",
        LabelAtomIndexQuery
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:validate-label-ontology-action-path:v1",
        "api/validate-label-ontology-action-path.v1.schema.json",
        "Validate Label Ontology Action Path v1",
        "api.validate-label-ontology-action.path",
        "schemas/fixtures/api/validate-label-ontology-action-path.v1.valid.json",
        "schemas/fixtures/api/validate-label-ontology-action-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:upsert-label-semantics-path:v1",
        "api/upsert-label-semantics-path.v1.schema.json",
        "Upsert Label Semantics Path v1",
        "api.upsert-label-semantics.path",
        "schemas/fixtures/api/upsert-label-semantics-path.v1.valid.json",
        "schemas/fixtures/api/upsert-label-semantics-path.v1.invalid.json",
        LabelSemanticsPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-path:v1",
        "api/revert-label-ontology-mutation-path.v1.schema.json",
        "Revert Label Ontology Mutation Path v1",
        "api.revert-label-ontology-mutation.path",
        "schemas/fixtures/api/revert-label-ontology-mutation-path.v1.valid.json",
        "schemas/fixtures/api/revert-label-ontology-mutation-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:delete-label-semantics-path:v1",
        "api/delete-label-semantics-path.v1.schema.json",
        "Delete Label Semantics Path v1",
        "api.delete-label-semantics.path",
        "schemas/fixtures/api/delete-label-semantics-path.v1.valid.json",
        "schemas/fixtures/api/delete-label-semantics-path.v1.invalid.json",
        LabelSemanticsPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:propose-task-label-path:v1",
        "api/propose-task-label-path.v1.schema.json",
        "Propose Task Label Path v1",
        "api.propose-task-label.path",
        "schemas/fixtures/api/propose-task-label-path.v1.valid.json",
        "schemas/fixtures/api/propose-task-label-path.v1.invalid.json",
        TaskLabelSurfacePath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:get-signal-path:v1",
        "api/get-signal-path.v1.schema.json",
        "Get Signal Path v1",
        "api.get-signal.path",
        "schemas/fixtures/api/get-signal-path.v1.valid.json",
        "schemas/fixtures/api/get-signal-path.v1.invalid.json",
        SignalPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:record-signal-path:v1",
        "api/record-signal-path.v1.schema.json",
        "Record Signal Path v1",
        "api.record-signal.path",
        "schemas/fixtures/api/record-signal-path.v1.valid.json",
        "schemas/fixtures/api/record-signal-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:confirm-signals-path:v1",
        "api/confirm-signals-path.v1.schema.json",
        "Confirm Signals Path v1",
        "api.confirm-signals.path",
        "schemas/fixtures/api/confirm-signals-path.v1.valid.json",
        "schemas/fixtures/api/confirm-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:reject-signals-path:v1",
        "api/reject-signals-path.v1.schema.json",
        "Reject Signals Path v1",
        "api.reject-signals.path",
        "schemas/fixtures/api/reject-signals-path.v1.valid.json",
        "schemas/fixtures/api/reject-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:resolve-signals-path:v1",
        "api/resolve-signals-path.v1.schema.json",
        "Resolve Signals Path v1",
        "api.resolve-signals.path",
        "schemas/fixtures/api/resolve-signals-path.v1.valid.json",
        "schemas/fixtures/api/resolve-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
    request_schema_root!(
        "urn:kanban-tool:schema:api:supersede-signals-path:v1",
        "api/supersede-signals-path.v1.schema.json",
        "Supersede Signals Path v1",
        "api.supersede-signals.path",
        "schemas/fixtures/api/supersede-signals-path.v1.valid.json",
        "schemas/fixtures/api/supersede-signals-path.v1.invalid.json",
        BoardLabelPath
    ),
];

pub fn generated_schema_ids() -> Vec<&'static str> {
    schema_registry().iter().map(|root| root.id).collect()
}

pub fn schema_registry() -> &'static [SchemaRoot] {
    static REGISTRY: std::sync::OnceLock<Vec<SchemaRoot>> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let mut registry = hybrid_static_schema_roots();
            registry.extend(protocol_schema_roots());
            registry.extend(portable_schema_roots());
            registry.extend(header_schema_roots());
            registry
        })
        .as_slice()
}

fn hybrid_static_schema_roots() -> Vec<SchemaRoot> {
    let board = crate::board_catalog::schema_roots();
    let mut registry = Vec::with_capacity(SCHEMA_REGISTRY.len() + 16);
    for root in SCHEMA_REGISTRY {
        match root.contract_id {
            "cli.board-use.output" => {
                append_board_schema_root(&mut registry, &board, "cli.board-list.output");
                append_board_schema_root(&mut registry, &board, "cli.board-create.output");
                append_board_schema_root(&mut registry, &board, "cli.board-show.output");
                registry.push(*root);
            }
            "cli.task-list.output" => {
                append_board_schema_root(&mut registry, &board, "cli.board-archive.output");
                append_board_schema_root(&mut registry, &board, "cli.board-columns.output");
                registry.push(*root);
            }
            "api.health.response" => {
                registry.push(*root);
                append_board_schema_root(&mut registry, &board, "api.list-boards.query");
                append_board_schema_root(&mut registry, &board, "api.create-board.request");
                append_board_schema_root(&mut registry, &board, "api.get-board.path");
                append_board_schema_root(&mut registry, &board, "api.archive-board.path");
                append_board_schema_root(&mut registry, &board, "api.list-boards.response");
                append_board_schema_root(&mut registry, &board, "api.create-board.response");
                append_board_schema_root(&mut registry, &board, "api.get-board.response");
                append_board_schema_root(&mut registry, &board, "api.archive-board.response");
            }
            "api.add-dependency.request" => {
                append_board_schema_root(&mut registry, &board, "api.archive-board.request");
                registry.push(*root);
            }
            "api.list-attachments.path" => {
                append_board_schema_root(&mut registry, &board, "api.list-board-columns.path");
                append_board_schema_root(&mut registry, &board, "api.list-board-columns.response");
                registry.push(*root);
            }
            _ => registry.push(*root),
        }
    }
    registry
}

fn append_board_schema_root(
    registry: &mut Vec<SchemaRoot>,
    board: &[SchemaRoot],
    contract_id: &str,
) {
    registry.push(
        *board
            .iter()
            .find(|root| root.contract_id == contract_id)
            .unwrap_or_else(|| panic!("missing board schema root: {contract_id}")),
    );
}

fn protocol_schema_roots() -> Vec<SchemaRoot> {
    vec![
        request_schema_root!(
            "urn:kanban-tool:schema:config:project-input:v1",
            "config/project-input.v1.schema.json",
            "Project Config Input v1",
            "config.project.input",
            "schemas/fixtures/config/project-input.v1.valid.json",
            "schemas/fixtures/config/project-input.v1.invalid.json",
            crate::ProjectConfigInput
        ),
        request_schema_root!(
            "urn:kanban-tool:schema:config:selected-worker-profile-input:v1",
            "config/selected-worker-profile-input.v1.schema.json",
            "Selected Worker Profile Input v1",
            "config.selected-worker-profile.input",
            "schemas/fixtures/config/selected-worker-profile-input.v1.valid.json",
            "schemas/fixtures/config/selected-worker-profile-input.v1.invalid.json",
            crate::WorkerProfileInput
        ),
    ]
}

fn portable_schema_roots() -> Vec<SchemaRoot> {
    let mut roots = Vec::with_capacity(crate::portable_contract_catalog().len() * 2);

    macro_rules! add_portable_roots {
        ($discriminator:literal, $input:ty, $output:ty) => {{
            let descriptor = crate::portable_contract_catalog()
                .iter()
                .find(|descriptor| descriptor.discriminator == $discriminator)
                .expect("portable schema type must have a frozen descriptor");
            roots.push(SchemaRoot {
                id: descriptor.input.schema_id,
                artifact_path: concat!("jsonl/", $discriminator, "-input.v1.schema.json"),
                title: concat!("Kanban JSONL ", $discriminator, " input v1"),
                contract_id: descriptor.input.contract_id,
                direction: ContractDirection::Deserialize,
                strictness: ContractStrictness::DenyUnknownFields,
                valid_fixture: descriptor.input.fixture,
                invalid_fixture: descriptor.input.invalid_fixture,
                generate: generate_for::<$input>,
            });
            roots.push(SchemaRoot {
                id: descriptor.output.schema_id,
                artifact_path: concat!("jsonl/", $discriminator, "-output.v1.schema.json"),
                title: concat!("Kanban JSONL ", $discriminator, " output v1"),
                contract_id: descriptor.output.contract_id,
                direction: ContractDirection::Serialize,
                strictness: ContractStrictness::DenyUnknownFields,
                valid_fixture: descriptor.output.fixture,
                invalid_fixture: descriptor.output.invalid_fixture,
                generate: generate_for::<$output>,
            });
        }};
    }

    add_portable_roots!(
        "board",
        crate::jsonl_core::BoardJsonlInput,
        crate::jsonl_core::BoardJsonlOutput
    );
    add_portable_roots!(
        "column",
        crate::jsonl_core::ColumnJsonlInput,
        crate::jsonl_core::ColumnJsonlOutput
    );
    add_portable_roots!(
        "task",
        crate::jsonl_core::TaskJsonlInput,
        crate::jsonl_core::TaskJsonlOutput
    );
    add_portable_roots!(
        "dependency",
        crate::jsonl_core::DependencyJsonlInput,
        crate::jsonl_core::DependencyJsonlOutput
    );
    add_portable_roots!(
        "run",
        crate::jsonl_core::RunJsonlInput,
        crate::jsonl_core::RunJsonlOutput
    );
    add_portable_roots!(
        "comment",
        crate::jsonl_core::CommentJsonlInput,
        crate::jsonl_core::CommentJsonlOutput
    );
    add_portable_roots!(
        "signal_observation",
        crate::jsonl_ledger::SignalObservationInput,
        crate::jsonl_ledger::SignalObservationOutput
    );
    add_portable_roots!(
        "signal",
        crate::jsonl_ledger::SignalInput,
        crate::jsonl_ledger::SignalOutput
    );
    add_portable_roots!(
        "event",
        crate::jsonl_core::EventJsonlInput,
        crate::jsonl_core::EventJsonlOutput
    );
    add_portable_roots!(
        "attachment",
        crate::jsonl_core::AttachmentJsonlInput,
        crate::jsonl_core::AttachmentJsonlOutput
    );
    add_portable_roots!(
        "label",
        crate::jsonl_ledger::LabelInput,
        crate::jsonl_ledger::LabelOutput
    );
    add_portable_roots!(
        "label_semantics",
        crate::jsonl_ledger::LabelSemanticsInput,
        crate::jsonl_ledger::LabelSemanticsOutput
    );
    add_portable_roots!(
        "label_atom",
        crate::jsonl_ledger::LabelAtomInput,
        crate::jsonl_ledger::LabelAtomOutput
    );
    add_portable_roots!(
        "label_semantic_proposal",
        crate::jsonl_ledger::LabelSemanticProposalInput,
        crate::jsonl_ledger::LabelSemanticProposalOutput
    );
    add_portable_roots!(
        "label_ontology_observation",
        crate::jsonl_ledger::LabelOntologyObservationInput,
        crate::jsonl_ledger::LabelOntologyObservationOutput
    );
    add_portable_roots!(
        "label_ontology_signal",
        crate::jsonl_ledger::LabelOntologySignalInput,
        crate::jsonl_ledger::LabelOntologySignalOutput
    );
    add_portable_roots!(
        "label_ontology_action",
        crate::jsonl_ledger::LabelOntologyActionInput,
        crate::jsonl_ledger::LabelOntologyActionOutput
    );
    add_portable_roots!(
        "label_ontology_action_atom_effect",
        crate::jsonl_ledger::LabelOntologyActionAtomEffectInput,
        crate::jsonl_ledger::LabelOntologyActionAtomEffectOutput
    );
    add_portable_roots!(
        "label_ontology_action_signal",
        crate::jsonl_ledger::LabelOntologyActionSignalInput,
        crate::jsonl_ledger::LabelOntologyActionSignalOutput
    );
    add_portable_roots!(
        "task_label",
        crate::jsonl_core::TaskLabelJsonlInput,
        crate::jsonl_core::TaskLabelJsonlOutput
    );
    add_portable_roots!(
        "setting",
        crate::jsonl_ledger::SettingInput,
        crate::jsonl_ledger::SettingOutput
    );

    roots
}

fn header_schema_roots() -> Vec<SchemaRoot> {
    let board = crate::board_catalog::schema_roots();
    crate::headers::api_header_contract_specs()
        .into_iter()
        .map(|spec| {
            if let Some(root) = board
                .iter()
                .find(|root| root.contract_id == spec.contract_id)
            {
                return *root;
            }
            SchemaRoot {
                id: crate::headers::schema_id(spec.endpoint.operation_id),
                artifact_path: crate::headers::artifact_path(spec.endpoint.operation_id),
                title: Box::leak(
                    format!("Kanban {} request headers v1", spec.endpoint.operation_id)
                        .into_boxed_str(),
                ),
                contract_id: spec.contract_id,
                direction: ContractDirection::Deserialize,
                strictness: ContractStrictness::DenyUnknownFields,
                valid_fixture: crate::headers::fixture_path(spec.profile, true),
                invalid_fixture: crate::headers::fixture_path(spec.profile, false),
                generate: match spec.profile {
                    crate::headers::ApiHeaderProfile::Locale => {
                        generate_for::<crate::headers::LocaleHeaders>
                    }
                    crate::headers::ApiHeaderProfile::LocaleActor => {
                        generate_for::<crate::headers::LocaleActorHeaders>
                    }
                    crate::headers::ApiHeaderProfile::LocaleJson => {
                        generate_for::<crate::headers::LocaleJsonHeaders>
                    }
                    crate::headers::ApiHeaderProfile::LocaleActorJson => {
                        generate_for::<crate::headers::LocaleActorJsonHeaders>
                    }
                    crate::headers::ApiHeaderProfile::LocaleActorOptionalJson => {
                        generate_for::<crate::headers::LocaleActorOptionalJsonHeaders>
                    }
                },
            }
        })
        .collect()
}

pub fn generated_artifacts() -> BTreeMap<String, Vec<u8>> {
    schema_registry()
        .iter()
        .map(|root| (root.artifact_path.to_owned(), schema_document_bytes(root)))
        .collect()
}

pub fn schema_document(root: &SchemaRoot) -> Value {
    let mut schema = (root.generate)(root.direction);
    let object = schema
        .as_object_mut()
        .expect("schemars root schema must be a JSON object");
    object.insert("$id".to_owned(), Value::String(root.id.to_owned()));
    object.insert("title".to_owned(), Value::String(root.title.to_owned()));
    canonicalize(schema)
}

pub fn schema_document_bytes(root: &SchemaRoot) -> Vec<u8> {
    let mut bytes =
        serde_json::to_vec_pretty(&schema_document(root)).expect("JSON Schema must serialize");
    bytes.push(b'\n');
    bytes
}

fn generate_for<T: JsonSchema>(direction: ContractDirection) -> Value {
    let settings = match direction {
        ContractDirection::Serialize => SchemaSettings::draft2020_12().for_serialize(),
        ContractDirection::Deserialize => SchemaSettings::draft2020_12().for_deserialize(),
        ContractDirection::Bidirectional => {
            panic!("bidirectional operation must register separate input and output roots")
        }
    };
    let schema = settings.into_generator().into_root_schema_for::<T>();
    serde_json::to_value(schema).expect("schemars root schema must serialize")
}

pub fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = Map::new();
            for (key, value) in entries {
                canonical.insert(key, canonicalize(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_root_uses_explicit_draft_and_offline_id() {
        for root in schema_registry() {
            let schema = schema_document(root);
            assert_eq!(schema["$schema"], DRAFT_2020_12, "{}", root.id);
            assert_eq!(schema["$id"], root.id, "{}", root.id);
            assert!(root.id.starts_with("urn:kanban-tool:schema:"));
        }
    }

    #[test]
    fn generated_documents_only_contain_local_refs() {
        for root in schema_registry() {
            assert_local_refs(&schema_document(root), root.id);
        }
    }

    #[test]
    fn every_portable_input_key_is_required_and_fixture_nulls_remain_nullable() {
        let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

        for descriptor in crate::portable_contract_catalog() {
            let root = schema_registry()
                .iter()
                .find(|root| root.id == descriptor.input.schema_id)
                .unwrap_or_else(|| panic!("missing input root {}", descriptor.input.schema_id));
            let schema = schema_document(root);
            let data_schema = resolve_local_ref(&schema, &schema["properties"]["data"]);
            let fixture: Value = serde_json::from_slice(
                &std::fs::read(repository_root.join(root.valid_fixture))
                    .unwrap_or_else(|error| panic!("read {}: {error}", root.valid_fixture)),
            )
            .unwrap_or_else(|error| panic!("parse {}: {error}", root.valid_fixture));
            let data = fixture["data"]
                .as_object()
                .unwrap_or_else(|| panic!("{} must contain object data", root.valid_fixture));
            let required = data_schema["required"]
                .as_array()
                .unwrap_or_else(|| panic!("{} data schema must declare required", root.id))
                .iter()
                .map(|key| key.as_str().expect("required key must be a string"))
                .collect::<std::collections::BTreeSet<_>>();
            let fixture_keys = data
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                required, fixture_keys,
                "{} must require every data key",
                root.id
            );

            for (key, value) in data {
                if value.is_null() {
                    let property = resolve_local_ref(&schema, &data_schema["properties"][key]);
                    assert!(
                        schema_allows_null(&schema, property),
                        "{} data.{key} must accept explicit null: {property}",
                        root.id
                    );
                }
            }
        }
    }

    fn resolve_local_ref<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
        schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|reference| reference.strip_prefix('#'))
            .and_then(|pointer| root.pointer(pointer))
            .unwrap_or(schema)
    }

    fn schema_allows_null(root: &Value, schema: &Value) -> bool {
        let schema = resolve_local_ref(root, schema);
        schema.get("type").is_some_and(|types| {
            types == "null"
                || types
                    .as_array()
                    .is_some_and(|types| types.iter().any(|schema_type| schema_type == "null"))
        }) || ["anyOf", "oneOf"]
            .into_iter()
            .filter_map(|keyword| schema.get(keyword).and_then(Value::as_array))
            .flatten()
            .any(|branch| schema_allows_null(root, branch))
    }

    fn assert_local_refs(value: &Value, root_id: &str) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    assert!(
                        reference.starts_with("#/"),
                        "{root_id}: external ref {reference}"
                    );
                }
                for value in object.values() {
                    assert_local_refs(value, root_id);
                }
            }
            Value::Array(values) => {
                for value in values {
                    assert_local_refs(value, root_id);
                }
            }
            _ => {}
        }
    }
}
