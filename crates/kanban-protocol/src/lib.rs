#![doc = include_str!("../README.md")]

//! kanban-tool 的公开 wire contract 与离线 schema catalog。
//!
//! 该 crate 只拥有跨 adapter 的机器可读契约，不拥有 SQLite record、service input、
//! HTTP handler 或 CLI command。operation inventory 保存当前公开 surface、contract、
//! transport、schema、fixture 与明确 exclusion；真实 runtime 行为继续由各 adapter 的
//! 集成测试验证。schema model 生成由显式 `schema` feature 启用；离线校验、artifact
//! 管理和 CLI tooling 分别由独立的 `kanban-web-artifact` 与 `xtask` owner 负责；本 crate 只拥有
//! 跨 adapter 的 wire value contract 与 schema/catalog。
// 通用 signal DTO 由 HTTP、client、CLI 和 MCP 共享。

pub mod admin_catalog;
mod api_components;
mod attachments;
pub mod board_catalog;
mod boards;
mod cli;
pub mod cli_helpers;
pub mod cli_labels;
pub mod cli_labels_catalog;
pub mod cli_operator;
pub mod cli_shell_catalog;
// Queue CLI parent declarations stay in one source projection for inventory and schema users.
// Its tests also verify each legacy row is projected exactly once.
// Keep this module public so xtask can consume the same declaration source.
pub mod cli_queue_catalog;
mod comments;
pub mod contract_catalog;
mod create_task;
mod dependencies;
pub mod dependency_catalog;
mod derived;
mod endpoint;
pub mod event_payload;
mod events;
mod headers;
pub mod history_catalog;
mod inventory;
pub mod jsonl_core;
pub mod jsonl_ledger;
pub mod knowledge_catalog;
mod label_surfaces;
mod labels;
pub mod labels_catalog;
mod lifecycle;
mod mcp;
pub mod metadata_config_catalog;
mod ontology;
pub mod operation_catalog;
mod portable;
#[cfg(all(test, feature = "schema"))]
mod protocol_tests;
mod protocols;
mod runs;
mod runtime;
pub mod runtime_catalog;
mod signals;
mod sse;
pub mod step_catalog;
mod steps;
pub mod structured_metadata;
mod surface;
pub mod task_catalog;
mod task_core;
mod task_graph;
mod task_read;
mod transitions;
pub mod web_artifact;
mod wire;

#[cfg(all(test, feature = "schema"))]
mod lifecycle_tests;

pub use api_components::{
    ApiExecutionPlanState, ApiLabel, ApiTask, ApiTaskPriority, ApiTaskStatus,
    ListTasksByStatusData, ListTasksByStatusResponse, ListTasksResponse, ListTasksStatusWindow,
};
pub use attachments::{
    ApiAttachment, AttachmentDownloadResponse, CreateAttachmentPath, CreateAttachmentRequest,
    CreateAttachmentResponse, DeleteAttachmentPath, DeleteAttachmentResponse, GetAttachmentPath,
    ListAttachmentsPath, ListAttachmentsResponse,
};
pub use boards::{
    ApiBoard, ApiBoardColumn, ArchiveBoardPath, ArchiveBoardResponse, CreateBoardRequest,
    CreateBoardResponse, GetBoardPath, GetBoardResponse, ListBoardColumnsPath,
    ListBoardColumnsResponse, ListBoardsQuery, ListBoardsResponse,
};
pub use cli::{
    CliAttachmentAddOutput, CliAttachmentListOutput, CliAttachmentRemoveOutput, CliBackupOutput,
    CliBackupResult, CliBoardColumnsOutput, CliBoardConfigSelection, CliBoardCurrentOutput,
    CliBoardUseOutput, CliCheckpointOutput, CliCommentAddOutput, CliCommentListOutput,
    CliConfigShow, CliConfigShowOutput, CliConfigSource, CliDependencyAddOutput, CliDependencyEdge,
    CliDependencyListOutput, CliDependencyMutation, CliDependencyRemoveOutput,
    CliDependencySnapshot, CliDependencyTask, CliDerivedStatusOutput, CliDerivedStoreStatus,
    CliDoctorOutput, CliEntity, CliEntityListOutput, CliEntityShowOutput, CliEntityUpsertOutput,
    CliEvent, CliEventsOutput, CliIndexDoctorOutput, CliIndexStatusOutput, CliInitOutput,
    CliInitResult, CliLegacyProjectionRootKind, CliMachineOutput, CliMaintenanceLegacyCleanup,
    CliMaintenanceLegacyCleanupAction, CliMaintenanceLegacyCleanupApply,
    CliMaintenanceLegacyCleanupApplyOutput, CliMaintenanceLegacyCleanupInventory,
    CliMaintenanceLegacyCleanupInventoryOutput, CliMaintenanceLegacyCleanupOutput,
    CliMaintenanceLegacyCleanupRestore, CliMaintenanceLegacyCleanupRestoreOutput,
    CliMaintenanceLegacyCleanupRoot, CliMaintenanceLegacyCleanupVerify,
    CliMaintenanceLegacyCleanupVerifyOutput, CliMaintenanceMode, CliMaintenanceOwnerStatus,
    CliMaintenanceRebuildOutput, CliMaintenanceRun, CliMaintenanceRunOutput, CliMaintenanceStatus,
    CliMaintenanceStatusOutput, CliMaintenanceStoreFailureKind, CliMaintenanceStoreResult,
    CliMaintenanceStoreRun, CliOperationDescriptor, CliOutboxItem, CliOutboxListOutput,
    CliProjectionRuntimeAvailability, CliProjectionStoreStatus, CliResolvedConfigValue,
    CliResolvedLocaleValue, CliRunLog, CliRunLogsOutput, CliRunShowOutput, CliRunsOutput,
    CliStatsOutput, CliTaskArchiveOutput, CliTaskBlockOutput, CliTaskClaimOutput,
    CliTaskCompleteOutput, CliTaskCreateOutput, CliTaskDoneOutput, CliTaskHeartbeatOutput,
    CliTaskListOutput, CliTaskPromoteOutput, CliTaskReclaimOutput, CliTaskReleaseOutput,
    CliTaskReopenOutput, CliTaskReviewOutput, CliTaskShowOutput, CliTaskSpecifyOutput,
    CliTaskStartOutput, CliTaskStepAddOutput, CliTaskStepDoneOutput, CliTaskStepListOutput,
    CliTaskStepNotRequiredOutput, CliTaskStepRemoveOutput, CliTaskStepRemoveResult,
    CliTaskStepReopenOutput, CliTaskStepSkipOutput, CliTaskStepUpdateOutput, CliTaskUnblockOutput,
    CliTaskUpdateOutput, CliVacuumOutput, CliVacuumResult, cli_operation_catalog,
};
pub use cli_helpers::{CliContextBuildOutput, CliGraphMapOutput, CliGraphNeighborhoodOutput};
pub use comments::{
    ApiComment, CommentAuthorType, CommentKind, CreateCommentPath, CreateCommentRequest,
    CreateCommentResponse, ListCommentsPath, ListCommentsResponse,
};
pub use contract_catalog::{
    ContractDeclaration, McpExposure, McpPolicy, McpToolBinding, OperationDeclaration,
};
#[cfg(feature = "schema")]
pub use contract_catalog::{SchemaGenerator, generate_schema_for};
pub use create_task::{ApiCreateTaskStatus, CreateTaskPath, CreateTaskRequest, CreateTaskResponse};
pub use dependencies::{
    AddDependencyPath, AddDependencyResponse, ApiDependencies, ApiDependencyEdge,
    ApiDependencyTask, ListDependenciesPath, ListDependenciesResponse, RemoveDependencyPath,
    RemoveDependencyResponse,
};
pub use derived::{
    ApiRelation, ApiRelationProvenance, BlockedReasonCount, BoardQuery, BuildContextPath,
    BuildContextQuery, BuildContextResponse, ContextDiagnostic, ContextEvidence, ContextItem,
    ContextPack, ContextPolicy, ContextProviderStatus, EntityListQuery, EntityListResponse,
    EntityPath, EntityResponse, EntityUpsertRequest, GraphMaintenance, GraphMaintenanceResponse,
    GraphNeighborsQuery, GraphNeighborsResponse, GraphQueryQuery, GraphStatus, GraphStatusResponse,
    ListEventsQuery, QueueStats, SearchMeta, SearchPageMeta, SearchStatus, SearchStatusResponse,
    SearchTaskHit, SearchTaskStatusWindow, SearchTaskStatusWindows, SearchTasksByStatusResponse,
    SearchTasksData, SearchTasksQuery, SearchTasksResponse, StaleClaim, StatsResponse, StatusCount,
    VectorStatus, VectorStatusResponse,
};
pub use endpoint::{
    EndpointDescriptor, EndpointObligation, EndpointObligationKind, EndpointObligations,
    HttpMethod, endpoint_catalog, endpoint_descriptor, endpoint_obligation_todo_count,
    validate_contract_topology, validate_endpoint_catalog, validate_operation_contracts,
};
pub use events::ListEventsResponse;
pub use headers::{ApiHeaderContractSpec, ApiHeaderProfile, api_header_contract_specs};
pub use inventory::{
    ContractBinding, ContractDirection, ContractGranularity, ContractStrictness, ContractSurface,
    ContractTransport, HttpTransportLocation, OperationContract, WireParameter,
    WireParameterCardinality, operation_inventory,
};
pub use label_surfaces::*;
pub use labels::{
    AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, ListTaskLabelsPath,
    ListTaskLabelsResponse, RemoveTaskLabelPath, RemoveTaskLabelResponse,
};
pub use lifecycle::{
    AddDependencyRequest, ArchiveBoardRequest, ArchiveTaskRequest, BlockTaskRequest,
    ClaimTaskRequest, CompleteTaskRequest, HeartbeatTaskRequest, PromoteTaskRequest,
    ReclaimTargetStatus, ReclaimTaskRequest, ReleaseTaskRequest, ReopenTaskRequest,
    SpecifyTaskRequest, SubmitReviewTaskRequest, UnblockTaskRequest,
};
pub use mcp::{
    McpOperationClass, McpOperationDescriptor, McpOperationInvariant, McpPolicyProjection,
    McpProjectionError, McpToolBindingProjection, mcp_host_admin_operation_ids,
    mcp_operation_catalog, mcp_operation_descriptor, project_mcp_policy,
    validate_mcp_operation_catalog, validate_mcp_policy_projection,
};
pub use ontology::{LabelOntologySignalWire, LabelOntologySignalsResponse};
pub use operation_catalog::{
    CatalogProjection, OPERATION_DECLARATIONS, operation_catalog, project,
};
pub use portable::{
    PortableContractDescriptor, PortableContractLane, PortableContractSide,
    operation_declarations as portable_operation_declarations, portable_contract_catalog,
    portable_operation_contracts,
};
pub use protocols::*;
pub use runs::{
    ApiClaim, ApiRun, ApiRunLog, ApiRunStatus, GetRunLogPath, GetRunLogResponse, GetRunPath,
    GetRunResponse, ListRunsPath, ListRunsResponse,
};
pub use runtime::WebRuntimeConfig;
pub use signals::{
    ConfirmSignalsResponse, RecordSignalRequest, RecordSignalResponse, RejectSignalsResponse,
    ResolveSignalsResponse, ReviewSignalsRequest, SignalCommentRequest, SignalRecordResult,
    SupersedeSignalsResponse,
};
pub use sse::{
    MAX_SAFE_EVENT_CURSOR, SSE_HEARTBEAT_EVENT, STREAM_EVENT_ENVELOPE_FIELDS, SseHeartbeatData,
    StreamEventData, StreamEventsHeaders, StreamEventsQuery, TASK_SCOPED_EVENT_KINDS,
    parse_event_cursor, task_scoped_event_kind, validate_event_cursor,
};
pub use steps::{
    ApiExecutionPlan, ApiStepStatus, ApiTaskStep, ApiTaskSteps, CompleteStepPath,
    CompleteStepRequest, CompleteStepResponse, CreateStepPath, CreateStepRequest,
    CreateStepResponse, ListStepsPath, ListStepsResponse, MarkExecutionPlanNotRequiredPath,
    MarkExecutionPlanNotRequiredRequest, MarkExecutionPlanNotRequiredResponse, RemoveStepPath,
    RemoveStepResponse, ReopenStepPath, ReopenStepRequest, ReopenStepResponse, SkipStepPath,
    SkipStepRequest, SkipStepResponse, UpdateStepPath, UpdateStepRequest, UpdateStepResponse,
};
pub use surface::{SurfaceOperation, surface_operation_catalog, surface_operation_keys};
pub use task_core::{
    GetTaskDetailsResponse, GetTaskPath, GetTaskQuery, GetTaskResponse, TaskDetailAggregate,
    TaskDetailOntology, TaskOntologySignalSummary, TaskOntologySummary, UpdateTaskPath,
    UpdateTaskRequest, UpdateTaskResponse,
};
pub use task_graph::{
    ApiTaskGraphEdgeKind, ApiTaskGraphNodeRole, BoardTaskMap, BoardTaskMapPath, BoardTaskMapQuery,
    BoardTaskMapResponse, TaskGraphEdge, TaskGraphMeta, TaskGraphNode, TaskNeighborhood,
    TaskNeighborhoodPath, TaskNeighborhoodQuery, TaskNeighborhoodResponse,
};
pub use task_read::{
    DEFAULT_TASK_READ_LIMIT, ListTasksByStatusPath, ListTasksByStatusQuery, ListTasksPath,
    ListTasksQuery, MAX_TASK_READ_ASSIGNEE_CHARS, MAX_TASK_READ_LABEL_CHARS, MAX_TASK_READ_LABELS,
    MAX_TASK_READ_LIMIT, MAX_TASK_READ_PLAN_FILTERS, MAX_TASK_READ_PRIORITIES,
    MAX_TASK_READ_Q_CHARS, MAX_TASK_READ_QUERY_BYTES, MAX_TASK_READ_QUERY_PAIRS,
    MAX_TASK_READ_STATUSES, TaskReadLabel, TaskReadPlanFilter, TaskReadSort,
};
pub use transitions::{
    ArchiveTaskPath, ArchiveTaskResponse, BlockTaskPath, BlockTaskResponse, ClaimTaskPath,
    ClaimTaskResponse, CompleteTaskPath, CompleteTaskResponse, HeartbeatTaskPath,
    HeartbeatTaskResponse, PromoteTaskPath, PromoteTaskResponse, ReclaimTaskPath,
    ReclaimTaskResponse, ReleaseTaskPath, ReleaseTaskResponse, ReopenTaskPath, ReopenTaskResponse,
    SpecifyTaskPath, SpecifyTaskResponse, SubmitReviewTaskPath, SubmitReviewTaskResponse,
    UnblockTaskPath, UnblockTaskResponse,
};
pub use web_artifact::{
    WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
    WEB_ARTIFACT_MANIFEST_PATH, WEB_PROTOCOL_VERSION, WebArtifactError, WebArtifactFile,
    WebArtifactManifest, validate_web_artifact_manifest, web_artifact_build_id_for,
    web_artifact_build_preimage, web_artifact_file_from_bytes, web_artifact_sha256_for_bytes,
};
pub use wire::{
    ApiErrorCode, CreatedLabelsMeta, DataEnvelope, DecisionMetadata, DecisionOption,
    DeleteResponse, DeleteResult, ErrorBody, ErrorEnvelope, HealthReport, HealthResponse,
    LabelOntologyReviewMeta, LimitMeta, MetadataEnvelope, NextAfterMeta, OffsetPaginationMeta,
    OptionalMetadataEnvelope, SignalFilterMeta, TaskOntologyDetails, TaskOntologyDetailsMeta,
    TotalPaginationMeta,
};

#[cfg(feature = "schema")]
pub mod schema;

#[cfg(feature = "schema")]
pub use schema::{generated_artifacts, generated_schema_ids, schema_registry};
mod maintenance;
pub use maintenance::{
    BackupReport, BackupResponse, CheckpointReport, CheckpointResponse, DoctorDerivedStore,
    DoctorIssue, DoctorReport, DoctorResponse, ExportReport, ExportResponse, ImportReport,
    ImportResponse, LegacyImportReport, LegacyImportRequest, LegacyImportResponse,
    LegacyImportTableCount, MaintenanceImportRequest, MaintenanceOwnerStatus,
    MaintenancePathRequest, MaintenanceRebuildResponse, MaintenanceRunReport,
    MaintenanceRunRequest, MaintenanceRunResponse, MaintenanceStatusReport,
    MaintenanceStatusResponse, ProjectionStoreStatus, VacuumReport, VacuumResponse,
};
