import { ObjectService, FileService } from '../../generated/rpc/kanban/extensions/v1/workspace_pb';
import { createClient } from '@connectrpc/connect'
import { createGrpcWebTransport } from '@connectrpc/connect-web'
import { KanbanService } from '../../generated/rpc/kanban/v1/kanban_pb'
import { QueryService } from '../../generated/rpc/kanban/v1/query_pb'
import { createRpcFetch } from './endpoint'

/** 同一个 runtime 的查询、命令和持续更新共享 binary gRPC-Web transport。 */
export function createRpcClients(baseUrl: string, fetcher?: typeof globalThis.fetch, onMessageBytes?: (bytes: number) => void) {
  const transport = createGrpcWebTransport({
    baseUrl,
    useBinaryFormat: true,
    fetch: createRpcFetch(baseUrl, fetcher, onMessageBytes),
  })
  return {
    objects: createClient(ObjectService, transport),
    files: createClient(FileService, transport),
    business: createClient(KanbanService, transport),
    query: createClient(QueryService, transport),
  }
}
