// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
// 仅供契约 fixtures/validator 测试使用；生产入口不得导入此聚合模块。
import { ContractValidationError } from "./runtime";
import { ApiListBoardColumnsPathSchema, apiListBoardColumnsPathValidator } from "./contracts/api-list-board-columns-path";
import { ApiListBoardColumnsResponseSchema, apiListBoardColumnsResponseValidator } from "./contracts/api-list-board-columns-response";
import { ApiDoctorResponseSchema, apiDoctorResponseValidator } from "./contracts/api-doctor-response";
import { ApiErrorResponseSchema, apiErrorResponseValidator } from "./contracts/api-error-response";
import { ApiListBoardsQuerySchema, apiListBoardsQueryValidator } from "./contracts/api-list-boards-query";
import { ApiListBoardsResponseSchema, apiListBoardsResponseValidator } from "./contracts/api-list-boards-response";
import { ApiArchiveTaskRequestSchema, apiArchiveTaskRequestValidator } from "./contracts/api-archive-task-request";
import { ApiAddDependencyRequestSchema, apiAddDependencyRequestValidator } from "./contracts/api-add-dependency-request";
import { ApiListBoardsHeadersSchema, apiListBoardsHeadersValidator } from "./contracts/api-list-boards-headers";
import { ApiListBoardColumnsHeadersSchema, apiListBoardColumnsHeadersValidator } from "./contracts/api-list-board-columns-headers";
import { ApiListTasksPathSchema, apiListTasksPathValidator } from "./contracts/api-list-tasks-path";
import { ApiListTasksQuerySchema, apiListTasksQueryValidator } from "./contracts/api-list-tasks-query";
import { ApiListTasksHeadersSchema, apiListTasksHeadersValidator } from "./contracts/api-list-tasks-headers";
import { ApiListTasksResponseSchema, apiListTasksResponseValidator } from "./contracts/api-list-tasks-response";
import { ApiListTasksByStatusPathSchema, apiListTasksByStatusPathValidator } from "./contracts/api-list-tasks-by-status-path";
import { ApiListTasksByStatusQuerySchema, apiListTasksByStatusQueryValidator } from "./contracts/api-list-tasks-by-status-query";
import { ApiListTasksByStatusHeadersSchema, apiListTasksByStatusHeadersValidator } from "./contracts/api-list-tasks-by-status-headers";
import { ApiListTasksByStatusResponseSchema, apiListTasksByStatusResponseValidator } from "./contracts/api-list-tasks-by-status-response";
import { ApiCreateTaskPathSchema, apiCreateTaskPathValidator } from "./contracts/api-create-task-path";
import { ApiCreateTaskHeadersSchema, apiCreateTaskHeadersValidator } from "./contracts/api-create-task-headers";
import { ApiCreateTaskRequestSchema, apiCreateTaskRequestValidator } from "./contracts/api-create-task-request";
import { ApiCreateTaskResponseSchema, apiCreateTaskResponseValidator } from "./contracts/api-create-task-response";
import { ApiGetTaskPathSchema, apiGetTaskPathValidator } from "./contracts/api-get-task-path";
import { ApiGetTaskQuerySchema, apiGetTaskQueryValidator } from "./contracts/api-get-task-query";
import { ApiGetTaskHeadersSchema, apiGetTaskHeadersValidator } from "./contracts/api-get-task-headers";
import { ApiGetTaskResponseSchema, apiGetTaskResponseValidator } from "./contracts/api-get-task-response";
import { ApiUpdateTaskPathSchema, apiUpdateTaskPathValidator } from "./contracts/api-update-task-path";
import { ApiUpdateTaskHeadersSchema, apiUpdateTaskHeadersValidator } from "./contracts/api-update-task-headers";
import { ApiUpdateTaskRequestSchema, apiUpdateTaskRequestValidator } from "./contracts/api-update-task-request";
import { ApiUpdateTaskResponseSchema, apiUpdateTaskResponseValidator } from "./contracts/api-update-task-response";
import { ApiSpecifyTaskPathSchema, apiSpecifyTaskPathValidator } from "./contracts/api-specify-task-path";
import { ApiSpecifyTaskHeadersSchema, apiSpecifyTaskHeadersValidator } from "./contracts/api-specify-task-headers";
import { ApiSpecifyTaskRequestSchema, apiSpecifyTaskRequestValidator } from "./contracts/api-specify-task-request";
import { ApiSpecifyTaskResponseSchema, apiSpecifyTaskResponseValidator } from "./contracts/api-specify-task-response";
import { ApiPromoteTaskPathSchema, apiPromoteTaskPathValidator } from "./contracts/api-promote-task-path";
import { ApiPromoteTaskHeadersSchema, apiPromoteTaskHeadersValidator } from "./contracts/api-promote-task-headers";
import { ApiPromoteTaskRequestSchema, apiPromoteTaskRequestValidator } from "./contracts/api-promote-task-request";
import { ApiPromoteTaskResponseSchema, apiPromoteTaskResponseValidator } from "./contracts/api-promote-task-response";
import { ApiClaimTaskPathSchema, apiClaimTaskPathValidator } from "./contracts/api-claim-task-path";
import { ApiClaimTaskHeadersSchema, apiClaimTaskHeadersValidator } from "./contracts/api-claim-task-headers";
import { ApiClaimTaskRequestSchema, apiClaimTaskRequestValidator } from "./contracts/api-claim-task-request";
import { ApiClaimTaskResponseSchema, apiClaimTaskResponseValidator } from "./contracts/api-claim-task-response";
import { ApiHeartbeatTaskPathSchema, apiHeartbeatTaskPathValidator } from "./contracts/api-heartbeat-task-path";
import { ApiHeartbeatTaskHeadersSchema, apiHeartbeatTaskHeadersValidator } from "./contracts/api-heartbeat-task-headers";
import { ApiHeartbeatTaskRequestSchema, apiHeartbeatTaskRequestValidator } from "./contracts/api-heartbeat-task-request";
import { ApiHeartbeatTaskResponseSchema, apiHeartbeatTaskResponseValidator } from "./contracts/api-heartbeat-task-response";
import { ApiCompleteTaskPathSchema, apiCompleteTaskPathValidator } from "./contracts/api-complete-task-path";
import { ApiCompleteTaskHeadersSchema, apiCompleteTaskHeadersValidator } from "./contracts/api-complete-task-headers";
import { ApiCompleteTaskRequestSchema, apiCompleteTaskRequestValidator } from "./contracts/api-complete-task-request";
import { ApiCompleteTaskResponseSchema, apiCompleteTaskResponseValidator } from "./contracts/api-complete-task-response";
import { ApiSubmitReviewTaskPathSchema, apiSubmitReviewTaskPathValidator } from "./contracts/api-submit-review-task-path";
import { ApiSubmitReviewTaskHeadersSchema, apiSubmitReviewTaskHeadersValidator } from "./contracts/api-submit-review-task-headers";
import { ApiSubmitReviewTaskRequestSchema, apiSubmitReviewTaskRequestValidator } from "./contracts/api-submit-review-task-request";
import { ApiSubmitReviewTaskResponseSchema, apiSubmitReviewTaskResponseValidator } from "./contracts/api-submit-review-task-response";
import { ApiBlockTaskPathSchema, apiBlockTaskPathValidator } from "./contracts/api-block-task-path";
import { ApiBlockTaskHeadersSchema, apiBlockTaskHeadersValidator } from "./contracts/api-block-task-headers";
import { ApiBlockTaskRequestSchema, apiBlockTaskRequestValidator } from "./contracts/api-block-task-request";
import { ApiBlockTaskResponseSchema, apiBlockTaskResponseValidator } from "./contracts/api-block-task-response";
import { ApiUnblockTaskPathSchema, apiUnblockTaskPathValidator } from "./contracts/api-unblock-task-path";
import { ApiUnblockTaskHeadersSchema, apiUnblockTaskHeadersValidator } from "./contracts/api-unblock-task-headers";
import { ApiUnblockTaskRequestSchema, apiUnblockTaskRequestValidator } from "./contracts/api-unblock-task-request";
import { ApiUnblockTaskResponseSchema, apiUnblockTaskResponseValidator } from "./contracts/api-unblock-task-response";
import { ApiArchiveTaskPathSchema, apiArchiveTaskPathValidator } from "./contracts/api-archive-task-path";
import { ApiArchiveTaskHeadersSchema, apiArchiveTaskHeadersValidator } from "./contracts/api-archive-task-headers";
import { ApiArchiveTaskResponseSchema, apiArchiveTaskResponseValidator } from "./contracts/api-archive-task-response";
import { ApiListStepsPathSchema, apiListStepsPathValidator } from "./contracts/api-list-steps-path";
import { ApiListStepsHeadersSchema, apiListStepsHeadersValidator } from "./contracts/api-list-steps-headers";
import { ApiListStepsResponseSchema, apiListStepsResponseValidator } from "./contracts/api-list-steps-response";
import { ApiCreateStepPathSchema, apiCreateStepPathValidator } from "./contracts/api-create-step-path";
import { ApiCreateStepHeadersSchema, apiCreateStepHeadersValidator } from "./contracts/api-create-step-headers";
import { ApiCreateStepRequestSchema, apiCreateStepRequestValidator } from "./contracts/api-create-step-request";
import { ApiCreateStepResponseSchema, apiCreateStepResponseValidator } from "./contracts/api-create-step-response";
import { ApiMarkExecutionPlanNotRequiredPathSchema, apiMarkExecutionPlanNotRequiredPathValidator } from "./contracts/api-mark-execution-plan-not-required-path";
import { ApiMarkExecutionPlanNotRequiredHeadersSchema, apiMarkExecutionPlanNotRequiredHeadersValidator } from "./contracts/api-mark-execution-plan-not-required-headers";
import { ApiMarkExecutionPlanNotRequiredRequestSchema, apiMarkExecutionPlanNotRequiredRequestValidator } from "./contracts/api-mark-execution-plan-not-required-request";
import { ApiMarkExecutionPlanNotRequiredResponseSchema, apiMarkExecutionPlanNotRequiredResponseValidator } from "./contracts/api-mark-execution-plan-not-required-response";
import { ApiListDependenciesPathSchema, apiListDependenciesPathValidator } from "./contracts/api-list-dependencies-path";
import { ApiListDependenciesHeadersSchema, apiListDependenciesHeadersValidator } from "./contracts/api-list-dependencies-headers";
import { ApiListDependenciesResponseSchema, apiListDependenciesResponseValidator } from "./contracts/api-list-dependencies-response";
import { ApiAddDependencyPathSchema, apiAddDependencyPathValidator } from "./contracts/api-add-dependency-path";
import { ApiAddDependencyHeadersSchema, apiAddDependencyHeadersValidator } from "./contracts/api-add-dependency-headers";
import { ApiAddDependencyResponseSchema, apiAddDependencyResponseValidator } from "./contracts/api-add-dependency-response";
import { ApiRemoveDependencyPathSchema, apiRemoveDependencyPathValidator } from "./contracts/api-remove-dependency-path";
import { ApiRemoveDependencyHeadersSchema, apiRemoveDependencyHeadersValidator } from "./contracts/api-remove-dependency-headers";
import { ApiRemoveDependencyResponseSchema, apiRemoveDependencyResponseValidator } from "./contracts/api-remove-dependency-response";
import { ApiListRunsPathSchema, apiListRunsPathValidator } from "./contracts/api-list-runs-path";
import { ApiListRunsHeadersSchema, apiListRunsHeadersValidator } from "./contracts/api-list-runs-headers";
import { ApiListRunsResponseSchema, apiListRunsResponseValidator } from "./contracts/api-list-runs-response";
import { ApiGetRunLogPathSchema, apiGetRunLogPathValidator } from "./contracts/api-get-run-log-path";
import { ApiGetRunLogHeadersSchema, apiGetRunLogHeadersValidator } from "./contracts/api-get-run-log-headers";
import { ApiGetRunLogResponseSchema, apiGetRunLogResponseValidator } from "./contracts/api-get-run-log-response";
import { ApiListCommentsPathSchema, apiListCommentsPathValidator } from "./contracts/api-list-comments-path";
import { ApiListCommentsHeadersSchema, apiListCommentsHeadersValidator } from "./contracts/api-list-comments-headers";
import { ApiListCommentsResponseSchema, apiListCommentsResponseValidator } from "./contracts/api-list-comments-response";
import { ApiCreateCommentPathSchema, apiCreateCommentPathValidator } from "./contracts/api-create-comment-path";
import { ApiCreateCommentHeadersSchema, apiCreateCommentHeadersValidator } from "./contracts/api-create-comment-headers";
import { ApiCreateCommentRequestSchema, apiCreateCommentRequestValidator } from "./contracts/api-create-comment-request";
import { ApiCreateCommentResponseSchema, apiCreateCommentResponseValidator } from "./contracts/api-create-comment-response";
import { ApiListAttachmentsPathSchema, apiListAttachmentsPathValidator } from "./contracts/api-list-attachments-path";
import { ApiListAttachmentsHeadersSchema, apiListAttachmentsHeadersValidator } from "./contracts/api-list-attachments-headers";
import { ApiListAttachmentsResponseSchema, apiListAttachmentsResponseValidator } from "./contracts/api-list-attachments-response";
import { ApiCreateAttachmentPathSchema, apiCreateAttachmentPathValidator } from "./contracts/api-create-attachment-path";
import { ApiCreateAttachmentHeadersSchema, apiCreateAttachmentHeadersValidator } from "./contracts/api-create-attachment-headers";
import { ApiCreateAttachmentRequestSchema, apiCreateAttachmentRequestValidator } from "./contracts/api-create-attachment-request";
import { ApiCreateAttachmentResponseSchema, apiCreateAttachmentResponseValidator } from "./contracts/api-create-attachment-response";
import { ApiDownloadAttachmentPathSchema, apiDownloadAttachmentPathValidator } from "./contracts/api-download-attachment-path";
import { ApiDownloadAttachmentHeadersSchema, apiDownloadAttachmentHeadersValidator } from "./contracts/api-download-attachment-headers";
import { ApiDownloadAttachmentResponseSchema, apiDownloadAttachmentResponseValidator } from "./contracts/api-download-attachment-response";
import { ApiDeleteAttachmentPathSchema, apiDeleteAttachmentPathValidator } from "./contracts/api-delete-attachment-path";
import { ApiDeleteAttachmentHeadersSchema, apiDeleteAttachmentHeadersValidator } from "./contracts/api-delete-attachment-headers";
import { ApiDeleteAttachmentResponseSchema, apiDeleteAttachmentResponseValidator } from "./contracts/api-delete-attachment-response";
import { ApiListEventsQuerySchema, apiListEventsQueryValidator } from "./contracts/api-list-events-query";
import { ApiListEventsHeadersSchema, apiListEventsHeadersValidator } from "./contracts/api-list-events-headers";
import { ApiListEventsResponseSchema, apiListEventsResponseValidator } from "./contracts/api-list-events-response";
import { SseStreamEventsQuerySchema, sseStreamEventsQueryValidator } from "./contracts/sse-stream-events-query";
import { SseEventDataSchema, sseEventDataValidator } from "./contracts/sse-event-data";
import { ApiListTaskLabelsPathSchema, apiListTaskLabelsPathValidator } from "./contracts/api-list-task-labels-path";
import { ApiListTaskLabelsHeadersSchema, apiListTaskLabelsHeadersValidator } from "./contracts/api-list-task-labels-headers";
import { ApiListTaskLabelsResponseSchema, apiListTaskLabelsResponseValidator } from "./contracts/api-list-task-labels-response";
import { ApiAddTaskLabelPathSchema, apiAddTaskLabelPathValidator } from "./contracts/api-add-task-label-path";
import { ApiAddTaskLabelHeadersSchema, apiAddTaskLabelHeadersValidator } from "./contracts/api-add-task-label-headers";
import { ApiAddTaskLabelRequestSchema, apiAddTaskLabelRequestValidator } from "./contracts/api-add-task-label-request";
import { ApiAddTaskLabelResponseSchema, apiAddTaskLabelResponseValidator } from "./contracts/api-add-task-label-response";
import { ApiRemoveTaskLabelPathSchema, apiRemoveTaskLabelPathValidator } from "./contracts/api-remove-task-label-path";
import { ApiRemoveTaskLabelHeadersSchema, apiRemoveTaskLabelHeadersValidator } from "./contracts/api-remove-task-label-headers";
import { ApiRemoveTaskLabelResponseSchema, apiRemoveTaskLabelResponseValidator } from "./contracts/api-remove-task-label-response";
import { ApiLabelAtomPathSchema, apiLabelAtomPathValidator } from "./contracts/api-label-atom-path";
import { ApiExplainLabelAtomHeadersSchema, apiExplainLabelAtomHeadersValidator } from "./contracts/api-explain-label-atom-headers";
import { ApiExplainLabelAtomResponseSchema, apiExplainLabelAtomResponseValidator } from "./contracts/api-explain-label-atom-response";
import { ApiReviewSignalsPathSchema, apiReviewSignalsPathValidator } from "./contracts/api-review-signals-path";
import { ApiReviewSignalsQuerySchema, apiReviewSignalsQueryValidator } from "./contracts/api-review-signals-query";
import { ApiReviewSignalsHeadersSchema, apiReviewSignalsHeadersValidator } from "./contracts/api-review-signals-headers";
import { ApiReviewSignalsResponseSchema, apiReviewSignalsResponseValidator } from "./contracts/api-review-signals-response";
import { ApiGetSignalPathSchema, apiGetSignalPathValidator } from "./contracts/api-get-signal-path";
import { ApiGetSignalHeadersSchema, apiGetSignalHeadersValidator } from "./contracts/api-get-signal-headers";
import { ApiGetSignalResponseSchema, apiGetSignalResponseValidator } from "./contracts/api-get-signal-response";
import { ApiSuggestTaskLabelsPathSchema, apiSuggestTaskLabelsPathValidator } from "./contracts/api-suggest-task-labels-path";
import { ApiLabelSuggestionQuerySchema, apiLabelSuggestionQueryValidator } from "./contracts/api-label-suggestion-query";
import { ApiSuggestTaskLabelsHeadersSchema, apiSuggestTaskLabelsHeadersValidator } from "./contracts/api-suggest-task-labels-headers";
import { ApiSuggestTaskLabelsResponseSchema, apiSuggestTaskLabelsResponseValidator } from "./contracts/api-suggest-task-labels-response";
import { ApiListLabelOntologySignalsPathSchema, apiListLabelOntologySignalsPathValidator } from "./contracts/api-list-label-ontology-signals-path";
import { ApiLabelOntologySignalQuerySchema, apiLabelOntologySignalQueryValidator } from "./contracts/api-label-ontology-signal-query";
import { ApiListLabelOntologySignalsHeadersSchema, apiListLabelOntologySignalsHeadersValidator } from "./contracts/api-list-label-ontology-signals-headers";
import { ApiListLabelOntologySignalsResponseSchema, apiListLabelOntologySignalsResponseValidator } from "./contracts/api-list-label-ontology-signals-response";
import { ApiReviewLabelOntologyPathSchema, apiReviewLabelOntologyPathValidator } from "./contracts/api-review-label-ontology-path";
import { ApiLabelOntologyReviewQuerySchema, apiLabelOntologyReviewQueryValidator } from "./contracts/api-label-ontology-review-query";
import { ApiReviewLabelOntologyHeadersSchema, apiReviewLabelOntologyHeadersValidator } from "./contracts/api-review-label-ontology-headers";
import { ApiReviewLabelOntologyResponseSchema, apiReviewLabelOntologyResponseValidator } from "./contracts/api-review-label-ontology-response";
import { ApiCreateLabelOntologyActionPathSchema, apiCreateLabelOntologyActionPathValidator } from "./contracts/api-create-label-ontology-action-path";
import { ApiCreateLabelOntologyActionHeadersSchema, apiCreateLabelOntologyActionHeadersValidator } from "./contracts/api-create-label-ontology-action-headers";
import { ApiCreateLabelOntologyActionRequestSchema, apiCreateLabelOntologyActionRequestValidator } from "./contracts/api-create-label-ontology-action-request";
import { ApiCreateLabelOntologyActionResponseSchema, apiCreateLabelOntologyActionResponseValidator } from "./contracts/api-create-label-ontology-action-response";
import { ApiGetLabelOntologySignalPathSchema, apiGetLabelOntologySignalPathValidator } from "./contracts/api-get-label-ontology-signal-path";
import { ApiGetLabelOntologySignalHeadersSchema, apiGetLabelOntologySignalHeadersValidator } from "./contracts/api-get-label-ontology-signal-headers";
import { ApiGetLabelOntologySignalResponseSchema, apiGetLabelOntologySignalResponseValidator } from "./contracts/api-get-label-ontology-signal-response";
import { ApiBoardTaskMapPathSchema, apiBoardTaskMapPathValidator } from "./contracts/api-board-task-map-path";
import { ApiBoardTaskMapHeadersSchema, apiBoardTaskMapHeadersValidator } from "./contracts/api-board-task-map-headers";
import { ApiBoardTaskMapQuerySchema, apiBoardTaskMapQueryValidator } from "./contracts/api-board-task-map-query";
import { ApiBoardTaskMapResponseSchema, apiBoardTaskMapResponseValidator } from "./contracts/api-board-task-map-response";
import { ApiTaskNeighborhoodPathSchema, apiTaskNeighborhoodPathValidator } from "./contracts/api-task-neighborhood-path";
import { ApiTaskNeighborhoodHeadersSchema, apiTaskNeighborhoodHeadersValidator } from "./contracts/api-task-neighborhood-headers";
import { ApiTaskNeighborhoodQuerySchema, apiTaskNeighborhoodQueryValidator } from "./contracts/api-task-neighborhood-query";
import { ApiTaskNeighborhoodResponseSchema, apiTaskNeighborhoodResponseValidator } from "./contracts/api-task-neighborhood-response";
import { ApiSearchStatusQuerySchema, apiSearchStatusQueryValidator } from "./contracts/api-search-status-query";
import { ApiSearchStatusHeadersSchema, apiSearchStatusHeadersValidator } from "./contracts/api-search-status-headers";
import { ApiSearchStatusResponseSchema, apiSearchStatusResponseValidator } from "./contracts/api-search-status-response";
import { ApiHealthHeadersSchema, apiHealthHeadersValidator } from "./contracts/api-health-headers";
import { ApiHealthResponseSchema, apiHealthResponseValidator } from "./contracts/api-health-response";
import { ApiGetStatsQuerySchema, apiGetStatsQueryValidator } from "./contracts/api-get-stats-query";
import { ApiGetStatsHeadersSchema, apiGetStatsHeadersValidator } from "./contracts/api-get-stats-headers";
import { ApiGetStatsResponseSchema, apiGetStatsResponseValidator } from "./contracts/api-get-stats-response";
import { ApiDoctorHeadersSchema, apiDoctorHeadersValidator } from "./contracts/api-doctor-headers";
import { ApiCheckpointHeadersSchema, apiCheckpointHeadersValidator } from "./contracts/api-checkpoint-headers";
import { ApiCheckpointResponseSchema, apiCheckpointResponseValidator } from "./contracts/api-checkpoint-response";
import { ApiMaintenanceBackupHeadersSchema, apiMaintenanceBackupHeadersValidator } from "./contracts/api-maintenance-backup-headers";
import { ApiMaintenanceBackupRequestSchema, apiMaintenanceBackupRequestValidator } from "./contracts/api-maintenance-backup-request";
import { ApiMaintenanceBackupResponseSchema, apiMaintenanceBackupResponseValidator } from "./contracts/api-maintenance-backup-response";
import { ApiMaintenanceExportHeadersSchema, apiMaintenanceExportHeadersValidator } from "./contracts/api-maintenance-export-headers";
import { ApiMaintenanceExportRequestSchema, apiMaintenanceExportRequestValidator } from "./contracts/api-maintenance-export-request";
import { ApiMaintenanceExportResponseSchema, apiMaintenanceExportResponseValidator } from "./contracts/api-maintenance-export-response";
import { ApiMaintenanceImportHeadersSchema, apiMaintenanceImportHeadersValidator } from "./contracts/api-maintenance-import-headers";
import { ApiMaintenanceImportRequestSchema, apiMaintenanceImportRequestValidator } from "./contracts/api-maintenance-import-request";
import { ApiMaintenanceImportResponseSchema, apiMaintenanceImportResponseValidator } from "./contracts/api-maintenance-import-response";
import { ApiMaintenanceVacuumHeadersSchema, apiMaintenanceVacuumHeadersValidator } from "./contracts/api-maintenance-vacuum-headers";
import { ApiMaintenanceVacuumResponseSchema, apiMaintenanceVacuumResponseValidator } from "./contracts/api-maintenance-vacuum-response";
import { ApiMaintenanceStatusHeadersSchema, apiMaintenanceStatusHeadersValidator } from "./contracts/api-maintenance-status-headers";
import { ApiMaintenanceStatusResponseSchema, apiMaintenanceStatusResponseValidator } from "./contracts/api-maintenance-status-response";
import { ApiMaintenanceRunHeadersSchema, apiMaintenanceRunHeadersValidator } from "./contracts/api-maintenance-run-headers";
import { ApiMaintenanceRunRequestSchema, apiMaintenanceRunRequestValidator } from "./contracts/api-maintenance-run-request";
import { ApiMaintenanceRunResponseSchema, apiMaintenanceRunResponseValidator } from "./contracts/api-maintenance-run-response";
import { ApiMaintenanceRebuildHeadersSchema, apiMaintenanceRebuildHeadersValidator } from "./contracts/api-maintenance-rebuild-headers";
import { ApiMaintenanceRebuildRequestSchema, apiMaintenanceRebuildRequestValidator } from "./contracts/api-maintenance-rebuild-request";
import { ApiMaintenanceRebuildResponseSchema, apiMaintenanceRebuildResponseValidator } from "./contracts/api-maintenance-rebuild-response";
import { ApiMaintenanceCleanupHeadersSchema, apiMaintenanceCleanupHeadersValidator } from "./contracts/api-maintenance-cleanup-headers";
import { ApiMaintenanceCleanupRequestSchema, apiMaintenanceCleanupRequestValidator } from "./contracts/api-maintenance-cleanup-request";
import { ApiMaintenanceCleanupResponseSchema, apiMaintenanceCleanupResponseValidator } from "./contracts/api-maintenance-cleanup-response";
import { ApiMaintenanceImportV30HeadersSchema, apiMaintenanceImportV30HeadersValidator } from "./contracts/api-maintenance-import-v30-headers";
import { ApiMaintenanceImportV30RequestSchema, apiMaintenanceImportV30RequestValidator } from "./contracts/api-maintenance-import-v30-request";
import { ApiMaintenanceImportV30ResponseSchema, apiMaintenanceImportV30ResponseValidator } from "./contracts/api-maintenance-import-v30-response";
import { RuntimeWebConfigOutputSchema, runtimeWebConfigOutputValidator } from "./contracts/runtime-web-config-output";

export { ContractValidationError };

export const schemas = {
  "api.list-board-columns.path": ApiListBoardColumnsPathSchema,
  "api.list-board-columns.response": ApiListBoardColumnsResponseSchema,
  "api.doctor.response": ApiDoctorResponseSchema,
  "api.error.response": ApiErrorResponseSchema,
  "api.list-boards.query": ApiListBoardsQuerySchema,
  "api.list-boards.response": ApiListBoardsResponseSchema,
  "api.archive-task.request": ApiArchiveTaskRequestSchema,
  "api.add-dependency.request": ApiAddDependencyRequestSchema,
  "api.list-boards.headers": ApiListBoardsHeadersSchema,
  "api.list-board-columns.headers": ApiListBoardColumnsHeadersSchema,
  "api.list-tasks.path": ApiListTasksPathSchema,
  "api.list-tasks.query": ApiListTasksQuerySchema,
  "api.list-tasks.headers": ApiListTasksHeadersSchema,
  "api.list-tasks.response": ApiListTasksResponseSchema,
  "api.list-tasks-by-status.path": ApiListTasksByStatusPathSchema,
  "api.list-tasks-by-status.query": ApiListTasksByStatusQuerySchema,
  "api.list-tasks-by-status.headers": ApiListTasksByStatusHeadersSchema,
  "api.list-tasks-by-status.response": ApiListTasksByStatusResponseSchema,
  "api.create-task.path": ApiCreateTaskPathSchema,
  "api.create-task.headers": ApiCreateTaskHeadersSchema,
  "api.create-task.request": ApiCreateTaskRequestSchema,
  "api.create-task.response": ApiCreateTaskResponseSchema,
  "api.get-task.path": ApiGetTaskPathSchema,
  "api.get-task.query": ApiGetTaskQuerySchema,
  "api.get-task.headers": ApiGetTaskHeadersSchema,
  "api.get-task.response": ApiGetTaskResponseSchema,
  "api.update-task.path": ApiUpdateTaskPathSchema,
  "api.update-task.headers": ApiUpdateTaskHeadersSchema,
  "api.update-task.request": ApiUpdateTaskRequestSchema,
  "api.update-task.response": ApiUpdateTaskResponseSchema,
  "api.specify-task.path": ApiSpecifyTaskPathSchema,
  "api.specify-task.headers": ApiSpecifyTaskHeadersSchema,
  "api.specify-task.request": ApiSpecifyTaskRequestSchema,
  "api.specify-task.response": ApiSpecifyTaskResponseSchema,
  "api.promote-task.path": ApiPromoteTaskPathSchema,
  "api.promote-task.headers": ApiPromoteTaskHeadersSchema,
  "api.promote-task.request": ApiPromoteTaskRequestSchema,
  "api.promote-task.response": ApiPromoteTaskResponseSchema,
  "api.claim-task.path": ApiClaimTaskPathSchema,
  "api.claim-task.headers": ApiClaimTaskHeadersSchema,
  "api.claim-task.request": ApiClaimTaskRequestSchema,
  "api.claim-task.response": ApiClaimTaskResponseSchema,
  "api.heartbeat-task.path": ApiHeartbeatTaskPathSchema,
  "api.heartbeat-task.headers": ApiHeartbeatTaskHeadersSchema,
  "api.heartbeat-task.request": ApiHeartbeatTaskRequestSchema,
  "api.heartbeat-task.response": ApiHeartbeatTaskResponseSchema,
  "api.complete-task.path": ApiCompleteTaskPathSchema,
  "api.complete-task.headers": ApiCompleteTaskHeadersSchema,
  "api.complete-task.request": ApiCompleteTaskRequestSchema,
  "api.complete-task.response": ApiCompleteTaskResponseSchema,
  "api.submit-review-task.path": ApiSubmitReviewTaskPathSchema,
  "api.submit-review-task.headers": ApiSubmitReviewTaskHeadersSchema,
  "api.submit-review-task.request": ApiSubmitReviewTaskRequestSchema,
  "api.submit-review-task.response": ApiSubmitReviewTaskResponseSchema,
  "api.block-task.path": ApiBlockTaskPathSchema,
  "api.block-task.headers": ApiBlockTaskHeadersSchema,
  "api.block-task.request": ApiBlockTaskRequestSchema,
  "api.block-task.response": ApiBlockTaskResponseSchema,
  "api.unblock-task.path": ApiUnblockTaskPathSchema,
  "api.unblock-task.headers": ApiUnblockTaskHeadersSchema,
  "api.unblock-task.request": ApiUnblockTaskRequestSchema,
  "api.unblock-task.response": ApiUnblockTaskResponseSchema,
  "api.archive-task.path": ApiArchiveTaskPathSchema,
  "api.archive-task.headers": ApiArchiveTaskHeadersSchema,
  "api.archive-task.response": ApiArchiveTaskResponseSchema,
  "api.list-steps.path": ApiListStepsPathSchema,
  "api.list-steps.headers": ApiListStepsHeadersSchema,
  "api.list-steps.response": ApiListStepsResponseSchema,
  "api.create-step.path": ApiCreateStepPathSchema,
  "api.create-step.headers": ApiCreateStepHeadersSchema,
  "api.create-step.request": ApiCreateStepRequestSchema,
  "api.create-step.response": ApiCreateStepResponseSchema,
  "api.mark-execution-plan-not-required.path": ApiMarkExecutionPlanNotRequiredPathSchema,
  "api.mark-execution-plan-not-required.headers": ApiMarkExecutionPlanNotRequiredHeadersSchema,
  "api.mark-execution-plan-not-required.request": ApiMarkExecutionPlanNotRequiredRequestSchema,
  "api.mark-execution-plan-not-required.response": ApiMarkExecutionPlanNotRequiredResponseSchema,
  "api.list-dependencies.path": ApiListDependenciesPathSchema,
  "api.list-dependencies.headers": ApiListDependenciesHeadersSchema,
  "api.list-dependencies.response": ApiListDependenciesResponseSchema,
  "api.add-dependency.path": ApiAddDependencyPathSchema,
  "api.add-dependency.headers": ApiAddDependencyHeadersSchema,
  "api.add-dependency.response": ApiAddDependencyResponseSchema,
  "api.remove-dependency.path": ApiRemoveDependencyPathSchema,
  "api.remove-dependency.headers": ApiRemoveDependencyHeadersSchema,
  "api.remove-dependency.response": ApiRemoveDependencyResponseSchema,
  "api.list-runs.path": ApiListRunsPathSchema,
  "api.list-runs.headers": ApiListRunsHeadersSchema,
  "api.list-runs.response": ApiListRunsResponseSchema,
  "api.get-run-log.path": ApiGetRunLogPathSchema,
  "api.get-run-log.headers": ApiGetRunLogHeadersSchema,
  "api.get-run-log.response": ApiGetRunLogResponseSchema,
  "api.list-comments.path": ApiListCommentsPathSchema,
  "api.list-comments.headers": ApiListCommentsHeadersSchema,
  "api.list-comments.response": ApiListCommentsResponseSchema,
  "api.create-comment.path": ApiCreateCommentPathSchema,
  "api.create-comment.headers": ApiCreateCommentHeadersSchema,
  "api.create-comment.request": ApiCreateCommentRequestSchema,
  "api.create-comment.response": ApiCreateCommentResponseSchema,
  "api.list-attachments.path": ApiListAttachmentsPathSchema,
  "api.list-attachments.headers": ApiListAttachmentsHeadersSchema,
  "api.list-attachments.response": ApiListAttachmentsResponseSchema,
  "api.create-attachment.path": ApiCreateAttachmentPathSchema,
  "api.create-attachment.headers": ApiCreateAttachmentHeadersSchema,
  "api.create-attachment.request": ApiCreateAttachmentRequestSchema,
  "api.create-attachment.response": ApiCreateAttachmentResponseSchema,
  "api.download-attachment.path": ApiDownloadAttachmentPathSchema,
  "api.download-attachment.headers": ApiDownloadAttachmentHeadersSchema,
  "api.download-attachment.response": ApiDownloadAttachmentResponseSchema,
  "api.delete-attachment.path": ApiDeleteAttachmentPathSchema,
  "api.delete-attachment.headers": ApiDeleteAttachmentHeadersSchema,
  "api.delete-attachment.response": ApiDeleteAttachmentResponseSchema,
  "api.list-events.query": ApiListEventsQuerySchema,
  "api.list-events.headers": ApiListEventsHeadersSchema,
  "api.list-events.response": ApiListEventsResponseSchema,
  "sse.stream-events.query": SseStreamEventsQuerySchema,
  "sse.event.data": SseEventDataSchema,
  "api.list-task-labels.path": ApiListTaskLabelsPathSchema,
  "api.list-task-labels.headers": ApiListTaskLabelsHeadersSchema,
  "api.list-task-labels.response": ApiListTaskLabelsResponseSchema,
  "api.add-task-label.path": ApiAddTaskLabelPathSchema,
  "api.add-task-label.headers": ApiAddTaskLabelHeadersSchema,
  "api.add-task-label.request": ApiAddTaskLabelRequestSchema,
  "api.add-task-label.response": ApiAddTaskLabelResponseSchema,
  "api.remove-task-label.path": ApiRemoveTaskLabelPathSchema,
  "api.remove-task-label.headers": ApiRemoveTaskLabelHeadersSchema,
  "api.remove-task-label.response": ApiRemoveTaskLabelResponseSchema,
  "api.label-atom.path": ApiLabelAtomPathSchema,
  "api.explain-label-atom.headers": ApiExplainLabelAtomHeadersSchema,
  "api.explain-label-atom.response": ApiExplainLabelAtomResponseSchema,
  "api.review-signals.path": ApiReviewSignalsPathSchema,
  "api.review-signals.query": ApiReviewSignalsQuerySchema,
  "api.review-signals.headers": ApiReviewSignalsHeadersSchema,
  "api.review-signals.response": ApiReviewSignalsResponseSchema,
  "api.get-signal.path": ApiGetSignalPathSchema,
  "api.get-signal.headers": ApiGetSignalHeadersSchema,
  "api.get-signal.response": ApiGetSignalResponseSchema,
  "api.suggest-task-labels.path": ApiSuggestTaskLabelsPathSchema,
  "api.label-suggestion.query": ApiLabelSuggestionQuerySchema,
  "api.suggest-task-labels.headers": ApiSuggestTaskLabelsHeadersSchema,
  "api.suggest-task-labels.response": ApiSuggestTaskLabelsResponseSchema,
  "api.list-label-ontology-signals.path": ApiListLabelOntologySignalsPathSchema,
  "api.label-ontology-signal.query": ApiLabelOntologySignalQuerySchema,
  "api.list-label-ontology-signals.headers": ApiListLabelOntologySignalsHeadersSchema,
  "api.list-label-ontology-signals.response": ApiListLabelOntologySignalsResponseSchema,
  "api.review-label-ontology.path": ApiReviewLabelOntologyPathSchema,
  "api.label-ontology-review.query": ApiLabelOntologyReviewQuerySchema,
  "api.review-label-ontology.headers": ApiReviewLabelOntologyHeadersSchema,
  "api.review-label-ontology.response": ApiReviewLabelOntologyResponseSchema,
  "api.create-label-ontology-action.path": ApiCreateLabelOntologyActionPathSchema,
  "api.create-label-ontology-action.headers": ApiCreateLabelOntologyActionHeadersSchema,
  "api.create-label-ontology-action.request": ApiCreateLabelOntologyActionRequestSchema,
  "api.create-label-ontology-action.response": ApiCreateLabelOntologyActionResponseSchema,
  "api.get-label-ontology-signal.path": ApiGetLabelOntologySignalPathSchema,
  "api.get-label-ontology-signal.headers": ApiGetLabelOntologySignalHeadersSchema,
  "api.get-label-ontology-signal.response": ApiGetLabelOntologySignalResponseSchema,
  "api.board-task-map.path": ApiBoardTaskMapPathSchema,
  "api.board-task-map.headers": ApiBoardTaskMapHeadersSchema,
  "api.board-task-map.query": ApiBoardTaskMapQuerySchema,
  "api.board-task-map.response": ApiBoardTaskMapResponseSchema,
  "api.task-neighborhood.path": ApiTaskNeighborhoodPathSchema,
  "api.task-neighborhood.headers": ApiTaskNeighborhoodHeadersSchema,
  "api.task-neighborhood.query": ApiTaskNeighborhoodQuerySchema,
  "api.task-neighborhood.response": ApiTaskNeighborhoodResponseSchema,
  "api.search-status.query": ApiSearchStatusQuerySchema,
  "api.search-status.headers": ApiSearchStatusHeadersSchema,
  "api.search-status.response": ApiSearchStatusResponseSchema,
  "api.health.headers": ApiHealthHeadersSchema,
  "api.health.response": ApiHealthResponseSchema,
  "api.get-stats.query": ApiGetStatsQuerySchema,
  "api.get-stats.headers": ApiGetStatsHeadersSchema,
  "api.get-stats.response": ApiGetStatsResponseSchema,
  "api.doctor.headers": ApiDoctorHeadersSchema,
  "api.checkpoint.headers": ApiCheckpointHeadersSchema,
  "api.checkpoint.response": ApiCheckpointResponseSchema,
  "api.maintenance-backup.headers": ApiMaintenanceBackupHeadersSchema,
  "api.maintenance-backup.request": ApiMaintenanceBackupRequestSchema,
  "api.maintenance-backup.response": ApiMaintenanceBackupResponseSchema,
  "api.maintenance-export.headers": ApiMaintenanceExportHeadersSchema,
  "api.maintenance-export.request": ApiMaintenanceExportRequestSchema,
  "api.maintenance-export.response": ApiMaintenanceExportResponseSchema,
  "api.maintenance-import.headers": ApiMaintenanceImportHeadersSchema,
  "api.maintenance-import.request": ApiMaintenanceImportRequestSchema,
  "api.maintenance-import.response": ApiMaintenanceImportResponseSchema,
  "api.maintenance-vacuum.headers": ApiMaintenanceVacuumHeadersSchema,
  "api.maintenance-vacuum.response": ApiMaintenanceVacuumResponseSchema,
  "api.maintenance-status.headers": ApiMaintenanceStatusHeadersSchema,
  "api.maintenance-status.response": ApiMaintenanceStatusResponseSchema,
  "api.maintenance-run.headers": ApiMaintenanceRunHeadersSchema,
  "api.maintenance-run.request": ApiMaintenanceRunRequestSchema,
  "api.maintenance-run.response": ApiMaintenanceRunResponseSchema,
  "api.maintenance-rebuild.headers": ApiMaintenanceRebuildHeadersSchema,
  "api.maintenance-rebuild.request": ApiMaintenanceRebuildRequestSchema,
  "api.maintenance-rebuild.response": ApiMaintenanceRebuildResponseSchema,
  "api.maintenance-cleanup.headers": ApiMaintenanceCleanupHeadersSchema,
  "api.maintenance-cleanup.request": ApiMaintenanceCleanupRequestSchema,
  "api.maintenance-cleanup.response": ApiMaintenanceCleanupResponseSchema,
  "api.maintenance-import-v30.headers": ApiMaintenanceImportV30HeadersSchema,
  "api.maintenance-import-v30.request": ApiMaintenanceImportV30RequestSchema,
  "api.maintenance-import-v30.response": ApiMaintenanceImportV30ResponseSchema,
  "runtime.web-config.output": RuntimeWebConfigOutputSchema,
} as const;

export const validators = {
  "api.list-board-columns.path": apiListBoardColumnsPathValidator,
  "api.list-board-columns.response": apiListBoardColumnsResponseValidator,
  "api.doctor.response": apiDoctorResponseValidator,
  "api.error.response": apiErrorResponseValidator,
  "api.list-boards.query": apiListBoardsQueryValidator,
  "api.list-boards.response": apiListBoardsResponseValidator,
  "api.archive-task.request": apiArchiveTaskRequestValidator,
  "api.add-dependency.request": apiAddDependencyRequestValidator,
  "api.list-boards.headers": apiListBoardsHeadersValidator,
  "api.list-board-columns.headers": apiListBoardColumnsHeadersValidator,
  "api.list-tasks.path": apiListTasksPathValidator,
  "api.list-tasks.query": apiListTasksQueryValidator,
  "api.list-tasks.headers": apiListTasksHeadersValidator,
  "api.list-tasks.response": apiListTasksResponseValidator,
  "api.list-tasks-by-status.path": apiListTasksByStatusPathValidator,
  "api.list-tasks-by-status.query": apiListTasksByStatusQueryValidator,
  "api.list-tasks-by-status.headers": apiListTasksByStatusHeadersValidator,
  "api.list-tasks-by-status.response": apiListTasksByStatusResponseValidator,
  "api.create-task.path": apiCreateTaskPathValidator,
  "api.create-task.headers": apiCreateTaskHeadersValidator,
  "api.create-task.request": apiCreateTaskRequestValidator,
  "api.create-task.response": apiCreateTaskResponseValidator,
  "api.get-task.path": apiGetTaskPathValidator,
  "api.get-task.query": apiGetTaskQueryValidator,
  "api.get-task.headers": apiGetTaskHeadersValidator,
  "api.get-task.response": apiGetTaskResponseValidator,
  "api.update-task.path": apiUpdateTaskPathValidator,
  "api.update-task.headers": apiUpdateTaskHeadersValidator,
  "api.update-task.request": apiUpdateTaskRequestValidator,
  "api.update-task.response": apiUpdateTaskResponseValidator,
  "api.specify-task.path": apiSpecifyTaskPathValidator,
  "api.specify-task.headers": apiSpecifyTaskHeadersValidator,
  "api.specify-task.request": apiSpecifyTaskRequestValidator,
  "api.specify-task.response": apiSpecifyTaskResponseValidator,
  "api.promote-task.path": apiPromoteTaskPathValidator,
  "api.promote-task.headers": apiPromoteTaskHeadersValidator,
  "api.promote-task.request": apiPromoteTaskRequestValidator,
  "api.promote-task.response": apiPromoteTaskResponseValidator,
  "api.claim-task.path": apiClaimTaskPathValidator,
  "api.claim-task.headers": apiClaimTaskHeadersValidator,
  "api.claim-task.request": apiClaimTaskRequestValidator,
  "api.claim-task.response": apiClaimTaskResponseValidator,
  "api.heartbeat-task.path": apiHeartbeatTaskPathValidator,
  "api.heartbeat-task.headers": apiHeartbeatTaskHeadersValidator,
  "api.heartbeat-task.request": apiHeartbeatTaskRequestValidator,
  "api.heartbeat-task.response": apiHeartbeatTaskResponseValidator,
  "api.complete-task.path": apiCompleteTaskPathValidator,
  "api.complete-task.headers": apiCompleteTaskHeadersValidator,
  "api.complete-task.request": apiCompleteTaskRequestValidator,
  "api.complete-task.response": apiCompleteTaskResponseValidator,
  "api.submit-review-task.path": apiSubmitReviewTaskPathValidator,
  "api.submit-review-task.headers": apiSubmitReviewTaskHeadersValidator,
  "api.submit-review-task.request": apiSubmitReviewTaskRequestValidator,
  "api.submit-review-task.response": apiSubmitReviewTaskResponseValidator,
  "api.block-task.path": apiBlockTaskPathValidator,
  "api.block-task.headers": apiBlockTaskHeadersValidator,
  "api.block-task.request": apiBlockTaskRequestValidator,
  "api.block-task.response": apiBlockTaskResponseValidator,
  "api.unblock-task.path": apiUnblockTaskPathValidator,
  "api.unblock-task.headers": apiUnblockTaskHeadersValidator,
  "api.unblock-task.request": apiUnblockTaskRequestValidator,
  "api.unblock-task.response": apiUnblockTaskResponseValidator,
  "api.archive-task.path": apiArchiveTaskPathValidator,
  "api.archive-task.headers": apiArchiveTaskHeadersValidator,
  "api.archive-task.response": apiArchiveTaskResponseValidator,
  "api.list-steps.path": apiListStepsPathValidator,
  "api.list-steps.headers": apiListStepsHeadersValidator,
  "api.list-steps.response": apiListStepsResponseValidator,
  "api.create-step.path": apiCreateStepPathValidator,
  "api.create-step.headers": apiCreateStepHeadersValidator,
  "api.create-step.request": apiCreateStepRequestValidator,
  "api.create-step.response": apiCreateStepResponseValidator,
  "api.mark-execution-plan-not-required.path": apiMarkExecutionPlanNotRequiredPathValidator,
  "api.mark-execution-plan-not-required.headers": apiMarkExecutionPlanNotRequiredHeadersValidator,
  "api.mark-execution-plan-not-required.request": apiMarkExecutionPlanNotRequiredRequestValidator,
  "api.mark-execution-plan-not-required.response": apiMarkExecutionPlanNotRequiredResponseValidator,
  "api.list-dependencies.path": apiListDependenciesPathValidator,
  "api.list-dependencies.headers": apiListDependenciesHeadersValidator,
  "api.list-dependencies.response": apiListDependenciesResponseValidator,
  "api.add-dependency.path": apiAddDependencyPathValidator,
  "api.add-dependency.headers": apiAddDependencyHeadersValidator,
  "api.add-dependency.response": apiAddDependencyResponseValidator,
  "api.remove-dependency.path": apiRemoveDependencyPathValidator,
  "api.remove-dependency.headers": apiRemoveDependencyHeadersValidator,
  "api.remove-dependency.response": apiRemoveDependencyResponseValidator,
  "api.list-runs.path": apiListRunsPathValidator,
  "api.list-runs.headers": apiListRunsHeadersValidator,
  "api.list-runs.response": apiListRunsResponseValidator,
  "api.get-run-log.path": apiGetRunLogPathValidator,
  "api.get-run-log.headers": apiGetRunLogHeadersValidator,
  "api.get-run-log.response": apiGetRunLogResponseValidator,
  "api.list-comments.path": apiListCommentsPathValidator,
  "api.list-comments.headers": apiListCommentsHeadersValidator,
  "api.list-comments.response": apiListCommentsResponseValidator,
  "api.create-comment.path": apiCreateCommentPathValidator,
  "api.create-comment.headers": apiCreateCommentHeadersValidator,
  "api.create-comment.request": apiCreateCommentRequestValidator,
  "api.create-comment.response": apiCreateCommentResponseValidator,
  "api.list-attachments.path": apiListAttachmentsPathValidator,
  "api.list-attachments.headers": apiListAttachmentsHeadersValidator,
  "api.list-attachments.response": apiListAttachmentsResponseValidator,
  "api.create-attachment.path": apiCreateAttachmentPathValidator,
  "api.create-attachment.headers": apiCreateAttachmentHeadersValidator,
  "api.create-attachment.request": apiCreateAttachmentRequestValidator,
  "api.create-attachment.response": apiCreateAttachmentResponseValidator,
  "api.download-attachment.path": apiDownloadAttachmentPathValidator,
  "api.download-attachment.headers": apiDownloadAttachmentHeadersValidator,
  "api.download-attachment.response": apiDownloadAttachmentResponseValidator,
  "api.delete-attachment.path": apiDeleteAttachmentPathValidator,
  "api.delete-attachment.headers": apiDeleteAttachmentHeadersValidator,
  "api.delete-attachment.response": apiDeleteAttachmentResponseValidator,
  "api.list-events.query": apiListEventsQueryValidator,
  "api.list-events.headers": apiListEventsHeadersValidator,
  "api.list-events.response": apiListEventsResponseValidator,
  "sse.stream-events.query": sseStreamEventsQueryValidator,
  "sse.event.data": sseEventDataValidator,
  "api.list-task-labels.path": apiListTaskLabelsPathValidator,
  "api.list-task-labels.headers": apiListTaskLabelsHeadersValidator,
  "api.list-task-labels.response": apiListTaskLabelsResponseValidator,
  "api.add-task-label.path": apiAddTaskLabelPathValidator,
  "api.add-task-label.headers": apiAddTaskLabelHeadersValidator,
  "api.add-task-label.request": apiAddTaskLabelRequestValidator,
  "api.add-task-label.response": apiAddTaskLabelResponseValidator,
  "api.remove-task-label.path": apiRemoveTaskLabelPathValidator,
  "api.remove-task-label.headers": apiRemoveTaskLabelHeadersValidator,
  "api.remove-task-label.response": apiRemoveTaskLabelResponseValidator,
  "api.label-atom.path": apiLabelAtomPathValidator,
  "api.explain-label-atom.headers": apiExplainLabelAtomHeadersValidator,
  "api.explain-label-atom.response": apiExplainLabelAtomResponseValidator,
  "api.review-signals.path": apiReviewSignalsPathValidator,
  "api.review-signals.query": apiReviewSignalsQueryValidator,
  "api.review-signals.headers": apiReviewSignalsHeadersValidator,
  "api.review-signals.response": apiReviewSignalsResponseValidator,
  "api.get-signal.path": apiGetSignalPathValidator,
  "api.get-signal.headers": apiGetSignalHeadersValidator,
  "api.get-signal.response": apiGetSignalResponseValidator,
  "api.suggest-task-labels.path": apiSuggestTaskLabelsPathValidator,
  "api.label-suggestion.query": apiLabelSuggestionQueryValidator,
  "api.suggest-task-labels.headers": apiSuggestTaskLabelsHeadersValidator,
  "api.suggest-task-labels.response": apiSuggestTaskLabelsResponseValidator,
  "api.list-label-ontology-signals.path": apiListLabelOntologySignalsPathValidator,
  "api.label-ontology-signal.query": apiLabelOntologySignalQueryValidator,
  "api.list-label-ontology-signals.headers": apiListLabelOntologySignalsHeadersValidator,
  "api.list-label-ontology-signals.response": apiListLabelOntologySignalsResponseValidator,
  "api.review-label-ontology.path": apiReviewLabelOntologyPathValidator,
  "api.label-ontology-review.query": apiLabelOntologyReviewQueryValidator,
  "api.review-label-ontology.headers": apiReviewLabelOntologyHeadersValidator,
  "api.review-label-ontology.response": apiReviewLabelOntologyResponseValidator,
  "api.create-label-ontology-action.path": apiCreateLabelOntologyActionPathValidator,
  "api.create-label-ontology-action.headers": apiCreateLabelOntologyActionHeadersValidator,
  "api.create-label-ontology-action.request": apiCreateLabelOntologyActionRequestValidator,
  "api.create-label-ontology-action.response": apiCreateLabelOntologyActionResponseValidator,
  "api.get-label-ontology-signal.path": apiGetLabelOntologySignalPathValidator,
  "api.get-label-ontology-signal.headers": apiGetLabelOntologySignalHeadersValidator,
  "api.get-label-ontology-signal.response": apiGetLabelOntologySignalResponseValidator,
  "api.board-task-map.path": apiBoardTaskMapPathValidator,
  "api.board-task-map.headers": apiBoardTaskMapHeadersValidator,
  "api.board-task-map.query": apiBoardTaskMapQueryValidator,
  "api.board-task-map.response": apiBoardTaskMapResponseValidator,
  "api.task-neighborhood.path": apiTaskNeighborhoodPathValidator,
  "api.task-neighborhood.headers": apiTaskNeighborhoodHeadersValidator,
  "api.task-neighborhood.query": apiTaskNeighborhoodQueryValidator,
  "api.task-neighborhood.response": apiTaskNeighborhoodResponseValidator,
  "api.search-status.query": apiSearchStatusQueryValidator,
  "api.search-status.headers": apiSearchStatusHeadersValidator,
  "api.search-status.response": apiSearchStatusResponseValidator,
  "api.health.headers": apiHealthHeadersValidator,
  "api.health.response": apiHealthResponseValidator,
  "api.get-stats.query": apiGetStatsQueryValidator,
  "api.get-stats.headers": apiGetStatsHeadersValidator,
  "api.get-stats.response": apiGetStatsResponseValidator,
  "api.doctor.headers": apiDoctorHeadersValidator,
  "api.checkpoint.headers": apiCheckpointHeadersValidator,
  "api.checkpoint.response": apiCheckpointResponseValidator,
  "api.maintenance-backup.headers": apiMaintenanceBackupHeadersValidator,
  "api.maintenance-backup.request": apiMaintenanceBackupRequestValidator,
  "api.maintenance-backup.response": apiMaintenanceBackupResponseValidator,
  "api.maintenance-export.headers": apiMaintenanceExportHeadersValidator,
  "api.maintenance-export.request": apiMaintenanceExportRequestValidator,
  "api.maintenance-export.response": apiMaintenanceExportResponseValidator,
  "api.maintenance-import.headers": apiMaintenanceImportHeadersValidator,
  "api.maintenance-import.request": apiMaintenanceImportRequestValidator,
  "api.maintenance-import.response": apiMaintenanceImportResponseValidator,
  "api.maintenance-vacuum.headers": apiMaintenanceVacuumHeadersValidator,
  "api.maintenance-vacuum.response": apiMaintenanceVacuumResponseValidator,
  "api.maintenance-status.headers": apiMaintenanceStatusHeadersValidator,
  "api.maintenance-status.response": apiMaintenanceStatusResponseValidator,
  "api.maintenance-run.headers": apiMaintenanceRunHeadersValidator,
  "api.maintenance-run.request": apiMaintenanceRunRequestValidator,
  "api.maintenance-run.response": apiMaintenanceRunResponseValidator,
  "api.maintenance-rebuild.headers": apiMaintenanceRebuildHeadersValidator,
  "api.maintenance-rebuild.request": apiMaintenanceRebuildRequestValidator,
  "api.maintenance-rebuild.response": apiMaintenanceRebuildResponseValidator,
  "api.maintenance-cleanup.headers": apiMaintenanceCleanupHeadersValidator,
  "api.maintenance-cleanup.request": apiMaintenanceCleanupRequestValidator,
  "api.maintenance-cleanup.response": apiMaintenanceCleanupResponseValidator,
  "api.maintenance-import-v30.headers": apiMaintenanceImportV30HeadersValidator,
  "api.maintenance-import-v30.request": apiMaintenanceImportV30RequestValidator,
  "api.maintenance-import-v30.response": apiMaintenanceImportV30ResponseValidator,
  "runtime.web-config.output": runtimeWebConfigOutputValidator,
} as const;

export type GeneratedContractId = keyof typeof validators;

export function isGeneratedContractId(value: unknown): value is GeneratedContractId {
  return typeof value === "string" && Object.prototype.hasOwnProperty.call(validators, value);
}

export function validateContract(id: GeneratedContractId, value: unknown): boolean {
  if (!isGeneratedContractId(id)) throw new Error(`Unknown generated contract id: ${String(id)}`);
  return validators[id](value);
}

export { parseApiListBoardColumnsPath } from "./contracts/api-list-board-columns-path";
export { parseApiListBoardColumnsResponse } from "./contracts/api-list-board-columns-response";
export { parseApiDoctorResponse } from "./contracts/api-doctor-response";
export { parseApiErrorResponse } from "./contracts/api-error-response";
export { parseApiListBoardsQuery } from "./contracts/api-list-boards-query";
export { parseApiListBoardsResponse } from "./contracts/api-list-boards-response";
export { parseApiArchiveTaskRequest } from "./contracts/api-archive-task-request";
export { parseApiAddDependencyRequest } from "./contracts/api-add-dependency-request";
export { parseApiListBoardsHeaders } from "./contracts/api-list-boards-headers";
export { parseApiListBoardColumnsHeaders } from "./contracts/api-list-board-columns-headers";
export { parseApiListTasksPath } from "./contracts/api-list-tasks-path";
export { parseApiListTasksQuery } from "./contracts/api-list-tasks-query";
export { parseApiListTasksHeaders } from "./contracts/api-list-tasks-headers";
export { parseApiListTasksResponse } from "./contracts/api-list-tasks-response";
export { parseApiListTasksByStatusPath } from "./contracts/api-list-tasks-by-status-path";
export { parseApiListTasksByStatusQuery } from "./contracts/api-list-tasks-by-status-query";
export { parseApiListTasksByStatusHeaders } from "./contracts/api-list-tasks-by-status-headers";
export { parseApiListTasksByStatusResponse } from "./contracts/api-list-tasks-by-status-response";
export { parseApiCreateTaskPath } from "./contracts/api-create-task-path";
export { parseApiCreateTaskHeaders } from "./contracts/api-create-task-headers";
export { parseApiCreateTaskRequest } from "./contracts/api-create-task-request";
export { parseApiCreateTaskResponse } from "./contracts/api-create-task-response";
export { parseApiGetTaskPath } from "./contracts/api-get-task-path";
export { parseApiGetTaskQuery } from "./contracts/api-get-task-query";
export { parseApiGetTaskHeaders } from "./contracts/api-get-task-headers";
export { parseApiGetTaskResponse } from "./contracts/api-get-task-response";
export { parseApiUpdateTaskPath } from "./contracts/api-update-task-path";
export { parseApiUpdateTaskHeaders } from "./contracts/api-update-task-headers";
export { parseApiUpdateTaskRequest } from "./contracts/api-update-task-request";
export { parseApiUpdateTaskResponse } from "./contracts/api-update-task-response";
export { parseApiSpecifyTaskPath } from "./contracts/api-specify-task-path";
export { parseApiSpecifyTaskHeaders } from "./contracts/api-specify-task-headers";
export { parseApiSpecifyTaskRequest } from "./contracts/api-specify-task-request";
export { parseApiSpecifyTaskResponse } from "./contracts/api-specify-task-response";
export { parseApiPromoteTaskPath } from "./contracts/api-promote-task-path";
export { parseApiPromoteTaskHeaders } from "./contracts/api-promote-task-headers";
export { parseApiPromoteTaskRequest } from "./contracts/api-promote-task-request";
export { parseApiPromoteTaskResponse } from "./contracts/api-promote-task-response";
export { parseApiClaimTaskPath } from "./contracts/api-claim-task-path";
export { parseApiClaimTaskHeaders } from "./contracts/api-claim-task-headers";
export { parseApiClaimTaskRequest } from "./contracts/api-claim-task-request";
export { parseApiClaimTaskResponse } from "./contracts/api-claim-task-response";
export { parseApiHeartbeatTaskPath } from "./contracts/api-heartbeat-task-path";
export { parseApiHeartbeatTaskHeaders } from "./contracts/api-heartbeat-task-headers";
export { parseApiHeartbeatTaskRequest } from "./contracts/api-heartbeat-task-request";
export { parseApiHeartbeatTaskResponse } from "./contracts/api-heartbeat-task-response";
export { parseApiCompleteTaskPath } from "./contracts/api-complete-task-path";
export { parseApiCompleteTaskHeaders } from "./contracts/api-complete-task-headers";
export { parseApiCompleteTaskRequest } from "./contracts/api-complete-task-request";
export { parseApiCompleteTaskResponse } from "./contracts/api-complete-task-response";
export { parseApiSubmitReviewTaskPath } from "./contracts/api-submit-review-task-path";
export { parseApiSubmitReviewTaskHeaders } from "./contracts/api-submit-review-task-headers";
export { parseApiSubmitReviewTaskRequest } from "./contracts/api-submit-review-task-request";
export { parseApiSubmitReviewTaskResponse } from "./contracts/api-submit-review-task-response";
export { parseApiBlockTaskPath } from "./contracts/api-block-task-path";
export { parseApiBlockTaskHeaders } from "./contracts/api-block-task-headers";
export { parseApiBlockTaskRequest } from "./contracts/api-block-task-request";
export { parseApiBlockTaskResponse } from "./contracts/api-block-task-response";
export { parseApiUnblockTaskPath } from "./contracts/api-unblock-task-path";
export { parseApiUnblockTaskHeaders } from "./contracts/api-unblock-task-headers";
export { parseApiUnblockTaskRequest } from "./contracts/api-unblock-task-request";
export { parseApiUnblockTaskResponse } from "./contracts/api-unblock-task-response";
export { parseApiArchiveTaskPath } from "./contracts/api-archive-task-path";
export { parseApiArchiveTaskHeaders } from "./contracts/api-archive-task-headers";
export { parseApiArchiveTaskResponse } from "./contracts/api-archive-task-response";
export { parseApiListStepsPath } from "./contracts/api-list-steps-path";
export { parseApiListStepsHeaders } from "./contracts/api-list-steps-headers";
export { parseApiListStepsResponse } from "./contracts/api-list-steps-response";
export { parseApiCreateStepPath } from "./contracts/api-create-step-path";
export { parseApiCreateStepHeaders } from "./contracts/api-create-step-headers";
export { parseApiCreateStepRequest } from "./contracts/api-create-step-request";
export { parseApiCreateStepResponse } from "./contracts/api-create-step-response";
export { parseApiMarkExecutionPlanNotRequiredPath } from "./contracts/api-mark-execution-plan-not-required-path";
export { parseApiMarkExecutionPlanNotRequiredHeaders } from "./contracts/api-mark-execution-plan-not-required-headers";
export { parseApiMarkExecutionPlanNotRequiredRequest } from "./contracts/api-mark-execution-plan-not-required-request";
export { parseApiMarkExecutionPlanNotRequiredResponse } from "./contracts/api-mark-execution-plan-not-required-response";
export { parseApiListDependenciesPath } from "./contracts/api-list-dependencies-path";
export { parseApiListDependenciesHeaders } from "./contracts/api-list-dependencies-headers";
export { parseApiListDependenciesResponse } from "./contracts/api-list-dependencies-response";
export { parseApiAddDependencyPath } from "./contracts/api-add-dependency-path";
export { parseApiAddDependencyHeaders } from "./contracts/api-add-dependency-headers";
export { parseApiAddDependencyResponse } from "./contracts/api-add-dependency-response";
export { parseApiRemoveDependencyPath } from "./contracts/api-remove-dependency-path";
export { parseApiRemoveDependencyHeaders } from "./contracts/api-remove-dependency-headers";
export { parseApiRemoveDependencyResponse } from "./contracts/api-remove-dependency-response";
export { parseApiListRunsPath } from "./contracts/api-list-runs-path";
export { parseApiListRunsHeaders } from "./contracts/api-list-runs-headers";
export { parseApiListRunsResponse } from "./contracts/api-list-runs-response";
export { parseApiGetRunLogPath } from "./contracts/api-get-run-log-path";
export { parseApiGetRunLogHeaders } from "./contracts/api-get-run-log-headers";
export { parseApiGetRunLogResponse } from "./contracts/api-get-run-log-response";
export { parseApiListCommentsPath } from "./contracts/api-list-comments-path";
export { parseApiListCommentsHeaders } from "./contracts/api-list-comments-headers";
export { parseApiListCommentsResponse } from "./contracts/api-list-comments-response";
export { parseApiCreateCommentPath } from "./contracts/api-create-comment-path";
export { parseApiCreateCommentHeaders } from "./contracts/api-create-comment-headers";
export { parseApiCreateCommentRequest } from "./contracts/api-create-comment-request";
export { parseApiCreateCommentResponse } from "./contracts/api-create-comment-response";
export { parseApiListAttachmentsPath } from "./contracts/api-list-attachments-path";
export { parseApiListAttachmentsHeaders } from "./contracts/api-list-attachments-headers";
export { parseApiListAttachmentsResponse } from "./contracts/api-list-attachments-response";
export { parseApiCreateAttachmentPath } from "./contracts/api-create-attachment-path";
export { parseApiCreateAttachmentHeaders } from "./contracts/api-create-attachment-headers";
export { parseApiCreateAttachmentRequest } from "./contracts/api-create-attachment-request";
export { parseApiCreateAttachmentResponse } from "./contracts/api-create-attachment-response";
export { parseApiDownloadAttachmentPath } from "./contracts/api-download-attachment-path";
export { parseApiDownloadAttachmentHeaders } from "./contracts/api-download-attachment-headers";
export { parseApiDownloadAttachmentResponse } from "./contracts/api-download-attachment-response";
export { parseApiDeleteAttachmentPath } from "./contracts/api-delete-attachment-path";
export { parseApiDeleteAttachmentHeaders } from "./contracts/api-delete-attachment-headers";
export { parseApiDeleteAttachmentResponse } from "./contracts/api-delete-attachment-response";
export { parseApiListEventsQuery } from "./contracts/api-list-events-query";
export { parseApiListEventsHeaders } from "./contracts/api-list-events-headers";
export { parseApiListEventsResponse } from "./contracts/api-list-events-response";
export { parseSseStreamEventsQuery } from "./contracts/sse-stream-events-query";
export { parseSseEventData } from "./contracts/sse-event-data";
export { parseApiListTaskLabelsPath } from "./contracts/api-list-task-labels-path";
export { parseApiListTaskLabelsHeaders } from "./contracts/api-list-task-labels-headers";
export { parseApiListTaskLabelsResponse } from "./contracts/api-list-task-labels-response";
export { parseApiAddTaskLabelPath } from "./contracts/api-add-task-label-path";
export { parseApiAddTaskLabelHeaders } from "./contracts/api-add-task-label-headers";
export { parseApiAddTaskLabelRequest } from "./contracts/api-add-task-label-request";
export { parseApiAddTaskLabelResponse } from "./contracts/api-add-task-label-response";
export { parseApiRemoveTaskLabelPath } from "./contracts/api-remove-task-label-path";
export { parseApiRemoveTaskLabelHeaders } from "./contracts/api-remove-task-label-headers";
export { parseApiRemoveTaskLabelResponse } from "./contracts/api-remove-task-label-response";
export { parseApiLabelAtomPath } from "./contracts/api-label-atom-path";
export { parseApiExplainLabelAtomHeaders } from "./contracts/api-explain-label-atom-headers";
export { parseApiExplainLabelAtomResponse } from "./contracts/api-explain-label-atom-response";
export { parseApiReviewSignalsPath } from "./contracts/api-review-signals-path";
export { parseApiReviewSignalsQuery } from "./contracts/api-review-signals-query";
export { parseApiReviewSignalsHeaders } from "./contracts/api-review-signals-headers";
export { parseApiReviewSignalsResponse } from "./contracts/api-review-signals-response";
export { parseApiGetSignalPath } from "./contracts/api-get-signal-path";
export { parseApiGetSignalHeaders } from "./contracts/api-get-signal-headers";
export { parseApiGetSignalResponse } from "./contracts/api-get-signal-response";
export { parseApiSuggestTaskLabelsPath } from "./contracts/api-suggest-task-labels-path";
export { parseApiLabelSuggestionQuery } from "./contracts/api-label-suggestion-query";
export { parseApiSuggestTaskLabelsHeaders } from "./contracts/api-suggest-task-labels-headers";
export { parseApiSuggestTaskLabelsResponse } from "./contracts/api-suggest-task-labels-response";
export { parseApiListLabelOntologySignalsPath } from "./contracts/api-list-label-ontology-signals-path";
export { parseApiLabelOntologySignalQuery } from "./contracts/api-label-ontology-signal-query";
export { parseApiListLabelOntologySignalsHeaders } from "./contracts/api-list-label-ontology-signals-headers";
export { parseApiListLabelOntologySignalsResponse } from "./contracts/api-list-label-ontology-signals-response";
export { parseApiReviewLabelOntologyPath } from "./contracts/api-review-label-ontology-path";
export { parseApiLabelOntologyReviewQuery } from "./contracts/api-label-ontology-review-query";
export { parseApiReviewLabelOntologyHeaders } from "./contracts/api-review-label-ontology-headers";
export { parseApiReviewLabelOntologyResponse } from "./contracts/api-review-label-ontology-response";
export { parseApiCreateLabelOntologyActionPath } from "./contracts/api-create-label-ontology-action-path";
export { parseApiCreateLabelOntologyActionHeaders } from "./contracts/api-create-label-ontology-action-headers";
export { parseApiCreateLabelOntologyActionRequest } from "./contracts/api-create-label-ontology-action-request";
export { parseApiCreateLabelOntologyActionResponse } from "./contracts/api-create-label-ontology-action-response";
export { parseApiGetLabelOntologySignalPath } from "./contracts/api-get-label-ontology-signal-path";
export { parseApiGetLabelOntologySignalHeaders } from "./contracts/api-get-label-ontology-signal-headers";
export { parseApiGetLabelOntologySignalResponse } from "./contracts/api-get-label-ontology-signal-response";
export { parseApiBoardTaskMapPath } from "./contracts/api-board-task-map-path";
export { parseApiBoardTaskMapHeaders } from "./contracts/api-board-task-map-headers";
export { parseApiBoardTaskMapQuery } from "./contracts/api-board-task-map-query";
export { parseApiBoardTaskMapResponse } from "./contracts/api-board-task-map-response";
export { parseApiTaskNeighborhoodPath } from "./contracts/api-task-neighborhood-path";
export { parseApiTaskNeighborhoodHeaders } from "./contracts/api-task-neighborhood-headers";
export { parseApiTaskNeighborhoodQuery } from "./contracts/api-task-neighborhood-query";
export { parseApiTaskNeighborhoodResponse } from "./contracts/api-task-neighborhood-response";
export { parseApiSearchStatusQuery } from "./contracts/api-search-status-query";
export { parseApiSearchStatusHeaders } from "./contracts/api-search-status-headers";
export { parseApiSearchStatusResponse } from "./contracts/api-search-status-response";
export { parseApiHealthHeaders } from "./contracts/api-health-headers";
export { parseApiHealthResponse } from "./contracts/api-health-response";
export { parseApiGetStatsQuery } from "./contracts/api-get-stats-query";
export { parseApiGetStatsHeaders } from "./contracts/api-get-stats-headers";
export { parseApiGetStatsResponse } from "./contracts/api-get-stats-response";
export { parseApiDoctorHeaders } from "./contracts/api-doctor-headers";
export { parseApiCheckpointHeaders } from "./contracts/api-checkpoint-headers";
export { parseApiCheckpointResponse } from "./contracts/api-checkpoint-response";
export { parseApiMaintenanceBackupHeaders } from "./contracts/api-maintenance-backup-headers";
export { parseApiMaintenanceBackupRequest } from "./contracts/api-maintenance-backup-request";
export { parseApiMaintenanceBackupResponse } from "./contracts/api-maintenance-backup-response";
export { parseApiMaintenanceExportHeaders } from "./contracts/api-maintenance-export-headers";
export { parseApiMaintenanceExportRequest } from "./contracts/api-maintenance-export-request";
export { parseApiMaintenanceExportResponse } from "./contracts/api-maintenance-export-response";
export { parseApiMaintenanceImportHeaders } from "./contracts/api-maintenance-import-headers";
export { parseApiMaintenanceImportRequest } from "./contracts/api-maintenance-import-request";
export { parseApiMaintenanceImportResponse } from "./contracts/api-maintenance-import-response";
export { parseApiMaintenanceVacuumHeaders } from "./contracts/api-maintenance-vacuum-headers";
export { parseApiMaintenanceVacuumResponse } from "./contracts/api-maintenance-vacuum-response";
export { parseApiMaintenanceStatusHeaders } from "./contracts/api-maintenance-status-headers";
export { parseApiMaintenanceStatusResponse } from "./contracts/api-maintenance-status-response";
export { parseApiMaintenanceRunHeaders } from "./contracts/api-maintenance-run-headers";
export { parseApiMaintenanceRunRequest } from "./contracts/api-maintenance-run-request";
export { parseApiMaintenanceRunResponse } from "./contracts/api-maintenance-run-response";
export { parseApiMaintenanceRebuildHeaders } from "./contracts/api-maintenance-rebuild-headers";
export { parseApiMaintenanceRebuildRequest } from "./contracts/api-maintenance-rebuild-request";
export { parseApiMaintenanceRebuildResponse } from "./contracts/api-maintenance-rebuild-response";
export { parseApiMaintenanceCleanupHeaders } from "./contracts/api-maintenance-cleanup-headers";
export { parseApiMaintenanceCleanupRequest } from "./contracts/api-maintenance-cleanup-request";
export { parseApiMaintenanceCleanupResponse } from "./contracts/api-maintenance-cleanup-response";
export { parseApiMaintenanceImportV30Headers } from "./contracts/api-maintenance-import-v30-headers";
export { parseApiMaintenanceImportV30Request } from "./contracts/api-maintenance-import-v30-request";
export { parseApiMaintenanceImportV30Response } from "./contracts/api-maintenance-import-v30-response";
export { parseRuntimeWebConfigOutput } from "./contracts/runtime-web-config-output";
