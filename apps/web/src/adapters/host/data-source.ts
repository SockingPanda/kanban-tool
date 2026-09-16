import { readBoardDirectory } from './board-directory';
import type { WebRuntimeConfig } from '../../lib/runtime';
import type { WorkspaceDataSource } from '../../application/workspace/data-source';
import { createRpcTransport } from './rpc-transport';
import type { RpcTransportOptions } from '../../application/data/rpc-transport';
import { loadBoardReadModel } from "./board-read-model";
import { createSubscribedBoardQuery } from './board-query';
import { QueryRegistry } from './query-registry';
import { createRpcClients } from '../../lib/rpc/client';
import { sameOriginRpcBase } from '../../lib/rpc/endpoint';
import { isQueryCall } from '../../lib/rpc/query-codec';
import type { RpcTransport } from '../../application/data/rpc-transport';
import { loadExplorerBoardIdentity, loadBoardEvents, loadTaskListPage, loadTaskMap, loadTaskRuns, loadTaskInspector, loadTaskInspectorNeighborhood, loadTaskInspectorRuns, loadTaskInspectorEvents, loadTaskInspectorAttachments } from "./explorer-read-model";
import { createTaskMutationClient } from "./task-mutations";
import { createAttachmentDownloadClient } from "./attachment-download";
import { readQueryHealth } from "./health-read-model";
import { createMaintenanceApi } from "./maintenance-api";

export function createHostDataSource(runtime: WebRuntimeConfig, options: RpcTransportOptions = {}): WorkspaceDataSource {
  const unary = createRpcTransport(runtime, options);
  const query = createRpcClients(sameOriginRpcBase(runtime, options.documentBaseURI).href, options.fetcher).query;
  const registry = new QueryRegistry((request, signal) => query.watchQueries(request, { signal }), typeof window === 'undefined' ? undefined : window);
  const transport: RpcTransport = {
    call: request => isQueryCall(request) ? registry.read(request) : unary.call(request),
    recentEvents: (request, signal) => registry.read({ method: 'RecentEvents', query: request, signal }),
  };
  return {
    readBoardDirectory: signal => readBoardDirectory(transport, signal),
    loadBoardReadModel: (config, selector, readOptions = {}) => loadBoardReadModel(config, selector, {
      ...readOptions, dependencies: { transport, ...readOptions.dependencies },
    }),
    createBoardReadQuery: (config, selector, readOptions = {}) => createSubscribedBoardQuery(config, selector, {
      ...readOptions, dependencies: { transport, ...readOptions.dependencies },
    }, registry),
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
    createTaskMutationClient: (config, board, dependencies) => createTaskMutationClient(config, board, { transport, ...dependencies }),
    createAttachmentDownloadClient: (config, dependencies) => createAttachmentDownloadClient(config, { transport, ...dependencies }),
    readHealth: readOptions => readQueryHealth(transport, readOptions?.signal),
    createMaintenanceApi: (dependencies, config) => createMaintenanceApi({ transport, ...dependencies }, config ?? runtime),
  };
}
