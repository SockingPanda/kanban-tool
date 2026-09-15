import type { BoardOption } from '../../domain/board-directory';
import type { WebRuntimeConfig } from "../../lib/runtime";
import type { BoardReadModelOptions, BoardReadModel, BoardReadQuery } from "../data/board-read-model";
import type { ExplorerReadOptions, ExplorerBoardIdentity, BoardEventsReadModel, TaskListQueryState, ExplorerTaskListPage, TaskMapQueryOptions, ExplorerTaskMapReadModel, TaskRunsReadModel, TaskInspectorReadOptions, TaskInspectorReadModel } from "../data/explorer-read-model";
import type { ApiTaskNeighborhoodResponseContract } from "../../lib/api/generated/contracts/api-task-neighborhood-response";
import type { ApiListRunsResponseContract } from "../../lib/api/generated/contracts/api-list-runs-response";
import type { ApiListEventsResponseContract } from "../../lib/api/generated/contracts/api-list-events-response";
import type { ApiListAttachmentsResponseContract } from "../../lib/api/generated/contracts/api-list-attachments-response";
import type { CanonicalBoardSlug } from "../../domain/board-slug";
import type { TaskMutationDependencies, TaskMutationClient } from "../data/task-mutations";
import type { AttachmentDownloadDependencies, AttachmentDownloadClient } from "../data/attachment-download";
import type { HealthReadDependencies, HealthReport } from "../data/health-read-model";
import type { MaintenanceApiDependencies, MaintenanceApi } from "../data/maintenance-api";
import type { RpcTransport } from '../data/rpc-transport';
import type { SseTransport } from '../sync/contracts';
import type { BoardRealtimeSource } from "../realtime/source";

/** 按 Board 和查询条件读取数据；提交返回 canonical service 的确认结果。 */
export interface WorkspaceDataSource {
  readBoardDirectory(signal?: AbortSignal): Promise<readonly BoardOption[]>;
  readonly transport: RpcTransport;
  /** 配置 RPC source 后不再创建 SSE controller；禁止自动协议回退。 */
  readonly boardRealtime?: BoardRealtimeSource;
  /** 仅供尚未迁移的旧宿主使用。 */
  readonly streamTransport?: SseTransport;
  readonly streamUrl?: string;
  loadBoardReadModel(runtime: WebRuntimeConfig, selector?: string, options?: BoardReadModelOptions): Promise<BoardReadModel>;
  createBoardReadQuery(runtime: WebRuntimeConfig, selector?: string, options?: BoardReadModelOptions): BoardReadQuery;
  loadExplorerBoardIdentity(runtime: WebRuntimeConfig, selector?: string, options?: ExplorerReadOptions): Promise<ExplorerBoardIdentity>;
  loadBoardEvents(runtime: WebRuntimeConfig, selector?: string, options?: ExplorerReadOptions & { readonly taskId?: string | null }): Promise<BoardEventsReadModel>;
  loadTaskListPage(runtime: WebRuntimeConfig, selector: string, query: TaskListQueryState, options?: ExplorerReadOptions): Promise<ExplorerTaskListPage>;
  loadTaskMap(runtime: WebRuntimeConfig, selector: string, options?: ExplorerReadOptions & Partial<TaskMapQueryOptions> & { readonly boardIdentity?: ExplorerBoardIdentity }): Promise<ExplorerTaskMapReadModel>;
  loadTaskRuns(runtime: WebRuntimeConfig, taskId: string, options?: ExplorerReadOptions): Promise<TaskRunsReadModel>;
  loadTaskInspector(runtime: WebRuntimeConfig, selector: string, taskId: string, options?: TaskInspectorReadOptions): Promise<TaskInspectorReadModel>;
  loadTaskInspectorNeighborhood(runtime: WebRuntimeConfig, selector: string, taskId: string, options?: ExplorerReadOptions): Promise<ApiTaskNeighborhoodResponseContract["data"]>;
  loadTaskInspectorRuns(runtime: WebRuntimeConfig, selector: string, taskId: string, options?: ExplorerReadOptions): Promise<ApiListRunsResponseContract["data"]>;
  loadTaskInspectorEvents(runtime: WebRuntimeConfig, selector: string, taskId: string, options?: ExplorerReadOptions): Promise<ApiListEventsResponseContract["data"]>;
  loadTaskInspectorAttachments(runtime: WebRuntimeConfig, selector: string, taskId: string, options?: ExplorerReadOptions): Promise<ApiListAttachmentsResponseContract["data"]>;
  createTaskMutationClient(runtime: WebRuntimeConfig, activeBoard: CanonicalBoardSlug, dependencies?: TaskMutationDependencies): TaskMutationClient;
  createAttachmentDownloadClient(runtime: WebRuntimeConfig, dependencies?: AttachmentDownloadDependencies): AttachmentDownloadClient;
  readHealth(options?: HealthReadDependencies & { readonly runtime?: WebRuntimeConfig; readonly signal?: AbortSignal }): Promise<HealthReport>;
  createMaintenanceApi(dependencies?: MaintenanceApiDependencies, runtime?: WebRuntimeConfig): MaintenanceApi;
}
