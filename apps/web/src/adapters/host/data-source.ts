import { readBoardDirectory } from './board-directory';
import type { WebRuntimeConfig } from '../../lib/runtime';
import type { WorkspaceDataSource } from '../../application/workspace/data-source';
import { createRpcTransport } from './rpc-transport';
import type { RpcTransportOptions } from '../../application/data/rpc-transport';
import { createHostRealtime } from './realtime';
import { loadBoardReadModel, createBoardReadQuery } from "./board-read-model";
import { loadExplorerBoardIdentity, loadBoardEvents, loadTaskListPage, loadTaskMap, loadTaskRuns, loadTaskInspector, loadTaskInspectorNeighborhood, loadTaskInspectorRuns, loadTaskInspectorEvents, loadTaskInspectorAttachments } from "./explorer-read-model";
import { createTaskMutationClient } from "./task-mutations";
import { createAttachmentDownloadClient } from "./attachment-download";
import { readHealth } from "./health-read-model";
import { createMaintenanceApi } from "./maintenance-api";

export function createHostDataSource(runtime: WebRuntimeConfig, options: RpcTransportOptions = {}): WorkspaceDataSource {
  const transport = createRpcTransport(runtime, options);
  return {
    transport,
    readBoardDirectory: signal => readBoardDirectory(transport, signal),
    boardRealtime: createHostRealtime(runtime, options),
    loadBoardReadModel: (config, selector, readOptions = {}) => loadBoardReadModel(config, selector, {
      ...readOptions, dependencies: { transport, ...readOptions.dependencies },
    }),
    createBoardReadQuery: (config, selector, readOptions = {}) => createBoardReadQuery(config, selector, {
      ...readOptions, dependencies: { transport, ...readOptions.dependencies },
    }),
    loadExplorerBoardIdentity: (config, selector, readOptions) => loadExplorerBoardIdentity(config, selector, { transport, ...readOptions }),
    loadBoardEvents: (config, selector, readOptions) => loadBoardEvents(config, selector, { transport, ...readOptions }),
    loadTaskListPage: (config, selector, query, readOptions) => loadTaskListPage(config, selector, query, { transport, ...readOptions }),
    loadTaskMap: (config, selector, readOptions) => loadTaskMap(config, selector, { transport, ...readOptions }),
    loadTaskRuns: (config, taskId, readOptions) => loadTaskRuns(config, taskId, { transport, ...readOptions }),
    loadTaskInspector: (config, selector, taskId, readOptions) => loadTaskInspector(config, selector, taskId, { transport, ...readOptions }),
    loadTaskInspectorNeighborhood: (config, selector, taskId, readOptions) => loadTaskInspectorNeighborhood(config, selector, taskId, { transport, ...readOptions }),
    loadTaskInspectorRuns: (config, selector, taskId, readOptions) => loadTaskInspectorRuns(config, selector, taskId, { transport, ...readOptions }),
    loadTaskInspectorEvents: (config, selector, taskId, readOptions) => loadTaskInspectorEvents(config, selector, taskId, { transport, ...readOptions }),
    loadTaskInspectorAttachments: (config, selector, taskId, readOptions) => loadTaskInspectorAttachments(config, selector, taskId, { transport, ...readOptions }),
    createTaskMutationClient,
    createAttachmentDownloadClient,
    readHealth,
    createMaintenanceApi,
  };
}
