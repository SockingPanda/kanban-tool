import { create, fromBinary, toBinary } from '@bufbuild/protobuf'
import { describe, expect, test, vi } from 'vitest'
import { defaultTaskListQuery } from '../../application/data/explorer-read-model'
import { QueryFrameSchema, WatchQueriesRequestSchema, type QueryDefinition, type QueryFrame } from '../../generated/rpc/kanban/v1/query_pb'
import { grpcWebFrame, queryResultBytes, readyFrame, resultFrames } from '../../lib/rpc/query-test-support'
import type { WebRuntimeConfig } from '../../lib/runtime'
import { createHostDataSource } from './data-source'
import { observeRead } from '../../application/query/observe-read'

const runtime: WebRuntimeConfig = {
  apiBaseUrl: '', webBasePath: '/app/', actor: 'test', defaultBoard: 'default', serverVersion: '3.1.0', protocolVersion: 'v2', webBuildId: 'test',
}
const documentBaseURI = 'http://127.0.0.1:1421/app/'
const board = { id: 'b_default', slug: 'default', name: 'Default', description: null, created_at: 1, updated_at: 2, archived_at: null }
const endpoint = 'http://127.0.0.1:1421/kanban.v1.QueryService/WatchQueries'

async function queryResponse(url: string, init: RequestInit | undefined, replies: Record<string, unknown>, calls: QueryDefinition[]): Promise<Response> {
  if (!(init?.body instanceof Uint8Array)) throw new Error('缺少 binary 请求')
  const request = fromBinary(WatchQueriesRequestSchema, init.body.subarray(5))
  const frames: QueryFrame[] = []
  for (const query of request.queries) {
    calls.push(query)
    if (!query.query.case || !replies[query.query.case]) throw new Error(`未预期的 query ${query.query.case}`)
    frames.push(...await resultFrames(query.clientQueryId, queryResultBytes(query, replies[query.query.case])), readyFrame(query.clientQueryId))
  }
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const frame of frames) controller.enqueue(grpcWebFrame(toBinary(QueryFrameSchema, create(QueryFrameSchema, frame))))
      if (init.signal?.aborted) controller.close()
      else init.signal?.addEventListener('abort', () => controller.close(), { once: true })
    },
  })
  const result = new Response(stream, { headers: { 'content-type': 'application/grpc-web+proto' } })
  Object.defineProperty(result, 'url', { value: url })
  return result
}

describe('生产 Host 数据源', () => {
  test('实际读取使用完整 QueryResult，保留 URL 分页、筛选、排序、DTO 与有界最新事件窗口', async () => {
    const replies = {
      listBoards: { data: [board] },
      listBoardColumns: { data: [] },
      listTasks: { data: [], meta: { limit: 25, offset: 25, total: 0 } },
      listRuns: { data: [] },
      recentEvents: { data: [], meta: { next_after: 0 } },
      boardTaskMap: { data: { nodes: [], edges: [], meta: {
        depth: 0, context_depth: 1, generated_at: 1, node_count: 0, edge_count: 0, truncated: false,
        active_statuses: ['ready'], active_only: true, include_done_context: true,
        include_archived_context: false, hide_isolated: false, limit_nodes: 240,
      } } },
    }
    const calls: QueryDefinition[] = []
    const fetcher = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input)
      expect(url).toBe(endpoint)
      expect(init).toMatchObject({ method: 'POST', mode: 'same-origin', credentials: 'same-origin', redirect: 'error', cache: 'no-store' })
      expect(new Headers(init?.headers).get('content-type')).toBe('application/grpc-web+proto')
      return queryResponse(url, init, replies, calls)
    })
    const source = createHostDataSource(runtime, { fetcher, documentBaseURI })
    expect(await source.readBoardDirectory()).toEqual([{ id: 'b_default', slug: 'default', name: 'Default', archived: false }])
    expect(await source.loadBoardReadModel(runtime, 'default', { includeTasks: false })).toMatchObject({ identity: { canonicalBoardId: 'b_default' }, columns: [] })
    expect(await source.loadTaskListPage(runtime, 'default', { ...defaultTaskListQuery, status: ['ready'], search: '保留查询', page: 2, limit: 25 })).toMatchObject({ meta: { limit: 25, offset: 25, total: 0 } })
    expect(await source.loadTaskMap(runtime, 'default')).toMatchObject({ map: { nodes: [] } })
    expect(await source.loadTaskRuns(runtime, 't_one')).toMatchObject({ runs: [] })
    expect(await source.loadBoardEvents(runtime, 'default')).toMatchObject({ events: [] })
    const list = calls.find(query => query.query.case === 'listTasks')?.query
    expect(list?.value).toMatchObject({ board: 'default', q: '保留查询', limit: 25n, offset: 25n })
    const recent = calls.find(query => query.query.case === 'recentEvents')?.query
    expect(recent?.value).toMatchObject({ boardId: 'b_default', limit: 150 })
  })

  test('连接失败保持错误，用户重试仍走 QueryService，不退回 unary', async () => {
    let offline = true
    const fetcher = vi.fn<typeof fetch>(async (input, init) => {
      if (offline) throw new Error('离线')
      return queryResponse(String(input), init, { listBoards: { data: [board] } }, [])
    })
    const source = createHostDataSource(runtime, { fetcher, documentBaseURI })
    await expect(source.loadExplorerBoardIdentity(runtime, 'default')).rejects.toMatchObject({ name: 'ExplorerReadError', kind: 'offline' })
    offline = false
    await expect(source.loadExplorerBoardIdentity(runtime, 'default')).resolves.toMatchObject({ id: 'b_default' })
    expect(fetcher.mock.calls.map(([input]) => String(input))).toEqual(Array(2).fill(endpoint))
  })

  test('runtime prefix 进入同一个 QueryService endpoint', async () => {
    const fetcher = vi.fn<typeof fetch>((input, init) => queryResponse(String(input), init, { listBoards: { data: [board] } }, []))
    const source = createHostDataSource({ ...runtime, apiBaseUrl: '/gateway' }, { fetcher, documentBaseURI })
    await source.readBoardDirectory()
    expect(String(fetcher.mock.calls[0][0])).toBe('http://127.0.0.1:1421/gateway/kanban.v1.QueryService/WatchQueries')
  })

  test('已经取消的 board bootstrap 不启动查询或遗留等待者', async () => {
    const fetcher = vi.fn<typeof fetch>()
    const source = createHostDataSource(runtime, { fetcher, documentBaseURI })
    const controller = new AbortController()
    controller.abort()
    await expect(source.createBoardReadQuery(runtime).load(controller.signal)).rejects.toMatchObject({ name: 'AbortError' })
    expect(fetcher).not.toHaveBeenCalled()
  })
})

test('对象与附件读取复用同一 QueryRegistry，跨 53 位整数不丢失', async () => {
  const object = { id: 'obj_one', board_id: board.id, type_key: 'module', title: '模块', body: null, version: { object: 9007199254740993n, source: null }, created_at: 9007199254740993n, updated_at: 2, archived_at: null, properties: {} };
  const replies = { listBoards: { data: [board] }, listObjects: { items: [object], next: null }, getObjectCatalog: { version: 1, types: [], relations: [], properties: [], bindings: [], workflows: [], rollups: [] }, listObjectFiles: { items: [{ id: 'a_one', boardId: board.id, ownerId: object.id, filename: 'empty', contentType: '', sizeBytes: '0', sha256: '', createdBy: '原作者', createdAt: '9007199254740993', objectVersion: '1' }], nextId: '' } };
  const calls: QueryDefinition[] = [];
  let active = 0, peak = 0;
  const fetcher = vi.fn<typeof fetch>(async (input, init) => {
    expect(String(input)).toBe(endpoint);
    active++; peak = Math.max(peak, active);
    init?.signal?.addEventListener('abort', () => active--, { once: true });
    return queryResponse(String(input), init, replies, calls);
  });
  const source = createHostDataSource(runtime, { fetcher, documentBaseURI });
  const objects = source.createObjectClient(runtime), files = source.createAttachmentTransferClient(runtime, 'default');
  const controller = new AbortController(), next = vi.fn(), failed = vi.fn();
  try {
    observeRead(signal => Promise.all([source.readBoardDirectory(signal), objects.catalog(signal), objects.list({ board_id: board.id }, signal), files.list(object.id, signal)]), controller.signal, next, failed);
    await vi.waitFor(() => expect(next).toHaveBeenCalled());
    const result = next.mock.calls.at(-1)![0];
    expect(result[2].items[0].version.object).toBe(9007199254740993n);
    expect(result[3][0].created_at).toBe(9007199254740993n);
    expect(new Set(calls.map(query => query.query.case))).toEqual(new Set(['listBoards', 'getObjectCatalog', 'listObjects', 'listObjectFiles']));
    expect(peak).toBe(1); expect(failed).not.toHaveBeenCalled();
  } finally { controller.abort(); }
});
