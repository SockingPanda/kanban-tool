import { readBoardDirectory } from './board-directory';
import type { WebRuntimeConfig } from '../../lib/runtime';
import type { WorkspaceDataSource } from '../../application/workspace/data-source';
import { createHttpTransport, resolveHttpRequestURL } from './http-transport';
import { createFetchSseTransport } from './sse-transport';
import { loadBoardReadModel, createBoardReadQuery } from "./board-read-model";
import { loadExplorerBoardIdentity, loadBoardEvents, loadTaskListPage, loadTaskMap, loadTaskRuns, loadTaskInspector, loadTaskInspectorNeighborhood, loadTaskInspectorRuns, loadTaskInspectorEvents, loadTaskInspectorAttachments } from "./explorer-read-model";
import { createTaskMutationClient } from "./task-mutations";
import { createAttachmentDownloadClient } from "./attachment-download";
import { readHealth } from "./health-read-model";
import { createMaintenanceApi } from "./maintenance-api";

export function createHostDataSource(runtime: WebRuntimeConfig): WorkspaceDataSource {
  const transport = createHttpTransport(runtime);
  const stream = new URL(resolveHttpRequestURL(runtime, "/api/v1/stream/events"));
  return { transport, readBoardDirectory: signal => readBoardDirectory(transport, signal), streamTransport: createFetchSseTransport(), streamUrl: `${stream.pathname}${stream.search}${stream.hash}`,
    loadBoardReadModel, createBoardReadQuery, loadExplorerBoardIdentity, loadBoardEvents, loadTaskListPage, loadTaskMap, loadTaskRuns, loadTaskInspector, loadTaskInspectorNeighborhood, loadTaskInspectorRuns, loadTaskInspectorEvents, loadTaskInspectorAttachments, createTaskMutationClient, createAttachmentDownloadClient, readHealth, createMaintenanceApi };
}
