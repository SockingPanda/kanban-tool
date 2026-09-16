import { afterEach, describe, expect, test, vi } from 'vitest'
import { create } from '@bufbuild/protobuf'
import { observeRead, refreshRead } from '../../application/query/observe-read'
import { QueryFrameSchema, type QueryDefinition, type QueryFrame, type WatchQueriesRequest } from '../../generated/rpc/kanban/v1/query_pb'
import type { QueryCall } from '../../lib/rpc/query-codec'
import { cursor, queryResultBytes, QueryFrameQueue, readyFrame, resultFrames } from '../../lib/rpc/query-test-support'
import { QueryRegistry } from './query-registry'
import { loadTaskRuns } from './explorer-read-model'
import { RpcTransportError } from '../../application/data/rpc-transport'

const controllers: AbortController[] = []
afterEach(() => { for (const controller of controllers.splice(0)) controller.abort() })
function control() { const controller = new AbortController(); controllers.push(controller); return controller }
const call: QueryCall = { method: 'ListBoards', query: { include_archived: false } }
const board = { id: 'b_a', slug: 'a', name: '甲', description: null, created_at: 1, updated_at: 1, archived_at: null }
const payload = (name = '甲') => ({ data: [{ ...board, name }] })

type TestQueue = Pick<QueryFrameQueue, 'push' | 'fail' | 'stream'>

/** 直接解决 iterator.next()，精确控制其 continuation 与 registry 重连 microtask 的先后。 */
function directFrameQueue(): TestQueue {
  const frames: QueryFrame[] = []
  let waiting: { resolve: (frame: IteratorResult<QueryFrame>) => void; reject: (error: Error) => void } | null = null
  return {
    push: (...next) => {
      frames.push(...next)
      if (waiting && frames.length) { const pending = waiting; waiting = null; pending.resolve({ done: false, value: frames.shift()! }) }
    },
    fail: error => { waiting?.reject(error); waiting = null },
    stream: signal => {
      signal.addEventListener('abort', () => { waiting?.resolve({ done: true, value: undefined }); waiting = null }, { once: true })
      return { [Symbol.asyncIterator]: () => ({
        next: () => {
          if (signal.aborted) return Promise.resolve({ done: true as const, value: undefined })
          const frame = frames.shift()
          if (frame) return Promise.resolve({ done: false as const, value: frame })
          return new Promise<IteratorResult<QueryFrame>>((resolve, reject) => { waiting = { resolve, reject } })
        },
      }) }
    },
  }
}

function harness(createQueue: () => TestQueue = () => new QueryFrameQueue(), pageLifecycle?: EventTarget) {
  const connections: Array<{ request: WatchQueriesRequest; signal: AbortSignal; queue: TestQueue }> = []
  const registry = new QueryRegistry((request, signal) => {
    const queue = createQueue()
    connections.push({ request, signal, queue })
    return queue.stream(signal)
  }, pageLifecycle)
  const latest = () => connections.at(-1)!
  const subscribe = (request = call, refresh = false) => {
    const controller = control()
    const next = vi.fn()
    const failed = vi.fn()
    observeRead(signal => registry.read({ ...request, signal }).then(value => value.payload), controller.signal, next, failed, refresh)
    return { next, failed, controller }
  }
  const publish = async (definition: QueryDefinition, name: string, revision = 1n) => {
    const encoded = queryResultBytes(definition, payload(name))
    latest().queue.push(...await resultFrames(definition.clientQueryId, encoded, cursor(revision, definition.clientQueryId)), readyFrame(definition.clientQueryId, cursor(revision, definition.clientQueryId)))
    return { encoded, cursor: cursor(revision, definition.clientQueryId) }
  }
  return { registry, connections, latest, subscribe, publish }
}

describe('单连接 multiplex 查询 registry', () => {
  test('pagehide 同步中止连接并丢弃暂存，隐藏期间不重连，pageshow 从已提交 cursor 恢复', async () => {
    const lifecycle = new EventTarget()
    const h = harness(directFrameQueue, lifecycle)
    const reader = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const definition = h.latest().request.queries[0]
    const previous = await h.publish(definition, '已提交')
    await vi.waitFor(() => expect(reader.next).toHaveBeenCalledTimes(1))
    const old = h.latest()
    const frames = await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('暂存旧结果')), cursor(2n, definition.clientQueryId), previous)
    old.queue.push(...frames.slice(0, -1))
    await Promise.resolve()
    h.registry.retry()
    lifecycle.dispatchEvent(new Event('pagehide'))
    expect(old.signal.aborted).toBe(true)
    old.queue.push(frames.at(-1)!, readyFrame(definition.clientQueryId, cursor(2n, definition.clientQueryId)))
    await Promise.resolve()
    expect(h.connections).toHaveLength(1)
    expect(reader.next).toHaveBeenCalledTimes(1)
    lifecycle.dispatchEvent(new Event('pageshow'))
    await vi.waitFor(() => expect(h.connections).toHaveLength(2))
    expect(h.latest().request.queries[0]).toMatchObject({ refresh: true, resume: previous.cursor })
    const restored = cursor(3n, definition.clientQueryId)
    h.latest().queue.push(...await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('恢复后结果')), restored, previous), readyFrame(definition.clientQueryId, restored))
    await vi.waitFor(() => expect(reader.next).toHaveBeenLastCalledWith(payload('恢复后结果')))
    expect(reader.failed).not.toHaveBeenCalled()
    reader.controller.abort()
    await Promise.resolve()
    lifecycle.dispatchEvent(new Event('pagehide'))
    lifecycle.dispatchEvent(new Event('pageshow'))
    await Promise.resolve()
    expect(h.registry.activeCount).toBe(0)
    expect(h.connections).toHaveLength(2)
  })

  test('空 ID 只允许全连接 heartbeat，不发布内容且不能放行未知查询的数据帧', async () => {
    const h = harness()
    const reader = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const definition = h.latest().request.queries[0]
    h.latest().queue.push(create(QueryFrameSchema, { body: { case: 'heartbeat', value: {} } }))
    await h.publish(definition, '甲')
    await vi.waitFor(() => expect(reader.next).toHaveBeenCalledTimes(1))
    expect(reader.failed).not.toHaveBeenCalled()
    expect(h.connections).toHaveLength(1)
    h.latest().queue.push(readyFrame('', cursor(1n)))
    await vi.waitFor(() => expect(reader.failed).toHaveBeenCalledWith(expect.objectContaining({ kind: 'invalid_bytes' })))
    expect(reader.next).toHaveBeenCalledTimes(1)
  })

  test('同查询共享一个订阅，集合变化携 cursor 重连，外部变化只更新实际消费者', async () => {
    const h = harness()
    const first = h.subscribe()
    const second = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    expect(h.latest().request.queries).toHaveLength(1)
    const a = h.latest().request.queries[0]
    const previous = await h.publish(a, '甲')
    await vi.waitFor(() => expect(first.next).toHaveBeenCalledTimes(1))
    expect(second.next).toHaveBeenCalledTimes(1)
    const unrelated = h.subscribe({ method: 'ListBoards', query: { include_archived: true } })
    await vi.waitFor(() => expect(h.connections).toHaveLength(2))
    expect(h.connections[0].signal.aborted).toBe(true)
    expect(h.latest().request.queries.find(query => query.clientQueryId === a.clientQueryId)?.resume).toEqual(previous.cursor)
    const b = h.latest().request.queries.find(query => query.clientQueryId !== a.clientQueryId)!
    await h.publish(b, '乙')
    await vi.waitFor(() => expect(unrelated.next).toHaveBeenCalledTimes(1))
    h.latest().queue.push(...await resultFrames(a.clientQueryId, queryResultBytes(a, payload('外部写入')), cursor(2n, a.clientQueryId), previous))
    await vi.waitFor(() => expect(first.next).toHaveBeenCalledTimes(2))
    expect(second.next).toHaveBeenCalledTimes(2)
    expect(unrelated.next).toHaveBeenCalledTimes(1)
    expect(h.connections).toHaveLength(2)
    first.controller.abort()
    await Promise.resolve()
    expect(h.registry.activeCount).toBe(2)
    expect(h.connections).toHaveLength(2)
    second.controller.abort()
    unrelated.controller.abort()
    await Promise.resolve()
    expect(h.registry.activeCount).toBe(0)
    expect(h.latest().signal.aborted).toBe(true)
  })

  test('显式同步携 refresh，缓存与 end 都不能提前完成 Ready barrier，无变化 Ready 也可完成', async () => {
    const h = harness()
    const visible = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const definition = h.latest().request.queries[0]
    const previous = await h.publish(definition, '甲')
    await vi.waitFor(() => expect(visible.next).toHaveBeenCalledTimes(1))
    const refresh = h.subscribe(call, true)
    await vi.waitFor(() => expect(h.connections).toHaveLength(2))
    expect(h.latest().request.queries[0]).toMatchObject({ refresh: true, resume: previous.cursor })
    expect(refresh.next).not.toHaveBeenCalled()
    h.latest().queue.push(readyFrame(definition.clientQueryId, previous.cursor))
    await vi.waitFor(() => expect(refresh.next).toHaveBeenCalledTimes(1))
    refresh.controller.abort()
    const changed = h.subscribe(call, true)
    await vi.waitFor(() => expect(h.connections).toHaveLength(3))
    h.latest().queue.push(...await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('已写入')), cursor(2n, definition.clientQueryId), previous))
    await vi.waitFor(() => expect(h.latest().signal.aborted).toBe(false))
    expect(changed.next).not.toHaveBeenCalled()
    h.latest().queue.push(readyFrame(definition.clientQueryId, cursor(2n, definition.clientQueryId)))
    await vi.waitFor(() => expect(changed.next).toHaveBeenCalledWith(payload('已写入')))
  })

  test.each([false, true])('旧连接 Ready 已排队时不能确认新 refresh，前次请求 refresh=%s', async previousRefresh => {
    const h = harness(directFrameQueue)
    const visible = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const definition = h.latest().request.queries[0]
    const previous = await h.publish(definition, '旧结果')
    await vi.waitFor(() => expect(visible.next).toHaveBeenCalledTimes(1))
    const previousReader = previousRefresh ? h.subscribe(call, true) : null
    if (previousRefresh) await vi.waitFor(() => expect(h.connections).toHaveLength(2))
    const previousConnection = h.latest()
    const count = h.connections.length
    // push 已将旧 Ready 的 for-await continuation 排队；refresh 同步设置等待，connect 随后才执行。
    previousConnection.queue.push(readyFrame(definition.clientQueryId, previous.cursor))
    const refreshed = h.subscribe(call, true)
    await vi.waitFor(() => expect(h.connections).toHaveLength(count + 1))
    expect(previousConnection.signal.aborted).toBe(true)
    expect(h.latest().request.queries[0]).toMatchObject({ refresh: true, resume: previous.cursor })
    expect(refreshed.next).not.toHaveBeenCalled()
    if (previousReader) expect(previousReader.next).not.toHaveBeenCalled()
    const nextCursor = cursor(2n, definition.clientQueryId)
    h.latest().queue.push(...await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('写后新结果')), nextCursor, previous))
    await Promise.resolve()
    expect(refreshed.next).not.toHaveBeenCalled()
    h.latest().queue.push(readyFrame(definition.clientQueryId, nextCursor))
    await vi.waitFor(() => expect(refreshed.next).toHaveBeenCalledWith(payload('写后新结果')))
    expect(refreshed.next).toHaveBeenCalledTimes(1)
    if (previousReader) expect(previousReader.next).toHaveBeenCalledWith(payload('写后新结果'))
  })

  test('identity 切换和卸载丢弃旧连接的暂存帧，64 位 cursor 无 number 转换', async () => {
    const h = harness()
    const old = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const oldConnection = h.latest()
    const definition = oldConnection.request.queries[0]
    const frames = await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('迟到')), cursor(9_007_199_254_740_993n))
    oldConnection.queue.push(...frames.slice(0, -1))
    old.controller.abort()
    const next = h.subscribe({ method: 'ListBoards', query: { include_archived: true } })
    await vi.waitFor(() => expect(h.connections).toHaveLength(2))
    oldConnection.queue.push(frames.at(-1)!)
    const fresh = h.latest().request.queries[0]
    await h.publish(fresh, '新项目', 9_007_199_254_740_993n)
    await vi.waitFor(() => expect(next.next).toHaveBeenCalledWith(payload('新项目')))
    expect(old.next).not.toHaveBeenCalled()
    const extra = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(3))
    expect(h.latest().request.queries.find(query => query.clientQueryId === fresh.clientQueryId)?.resume?.revision).toBe(9_007_199_254_740_993n)
    extra.controller.abort()
  })

  test('分页和筛选属于查询身份，释放后不保留后台消费者', async () => {
    const h = harness()
    const pageOne = h.subscribe({ method: 'ListTasks', path: { board: 'a' }, query: { offset: 0, limit: 25, sort: 'title', status: ['ready'] } })
    const pageTwo = h.subscribe({ method: 'ListTasks', path: { board: 'a' }, query: { offset: 25, limit: 25, sort: 'title', status: ['ready'] } })
    await vi.waitFor(() => expect(h.latest().request.queries).toHaveLength(2))
    expect(h.latest().request.queries.map(query => query.query.case === 'listTasks' ? query.query.value.offset : null)).toEqual([0n, 25n])
    pageOne.controller.abort()
    pageTwo.controller.abort()
    await Promise.resolve()
    expect(h.registry.activeCount).toBe(0)
  })

  test('断流丢弃未完成 delta，重连仅携上次已提交 cursor，Host epoch 改变后接受完整快照', async () => {
    const h = harness()
    const reader = h.subscribe()
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const definition = h.latest().request.queries[0]
    const previous = await h.publish(definition, '旧结果')
    await vi.waitFor(() => expect(reader.next).toHaveBeenCalledTimes(1))
    const incomplete = await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('未提交')), cursor(2n, definition.clientQueryId), previous)
    h.latest().queue.push(...incomplete.slice(0, -1))
    h.latest().queue.fail(new RpcTransportError('offline', 'stream interrupted'))
    await vi.waitFor(() => expect(reader.failed).toHaveBeenCalledTimes(1))
    expect(reader.next).toHaveBeenCalledTimes(1)
    await vi.waitFor(() => expect(h.connections).toHaveLength(2))
    expect(h.latest().request.queries[0].resume).toEqual(previous.cursor)
    const newCursor = cursor(1n, definition.clientQueryId, 'new-host')
    h.latest().queue.push(...await resultFrames(definition.clientQueryId, queryResultBytes(definition, payload('Host 已重启')), newCursor), readyFrame(definition.clientQueryId, newCursor))
    await vi.waitFor(() => expect(reader.next).toHaveBeenLastCalledWith(payload('Host 已重启')))
  })

  test('活跃 query 数量有明确上限，拒绝第 65 个且卸载后全部释放', async () => {
    const h = harness()
    const readers = Array.from({ length: 65 }, (_, offset) => h.subscribe({ method: 'ListTasks', path: { board: 'a' }, query: { offset, limit: 1 } }))
    await vi.waitFor(() => expect(readers[64].failed).toHaveBeenCalledWith(expect.objectContaining({ kind: 'response_too_large' })))
    expect(h.registry.activeCount).toBe(64)
    expect(h.latest().request.queries).toHaveLength(64)
    for (const reader of readers) reader.controller.abort()
    await Promise.resolve()
    expect(h.registry.activeCount).toBe(0)
  })

  test('真实 Runs reader 订阅日志追加，选中 run 改变后原子映射并释放旧日志查询', async () => {
    const h = harness()
    const controller = control()
    const next = vi.fn()
    const failed = vi.fn()
    const run = (id: string) => ({ id, task_id: 't_1', status: 'succeeded', worker_profile: 'worker', worker_pid: null, claim_owner: 'owner', started_at: 1, finished_at: 2, exit_code: 0, summary: 'done', error: null, has_log: true, metadata: {} })
    const runtime = { apiBaseUrl: '', webBasePath: '/app/', actor: 'test', defaultBoard: 'a', serverVersion: '3.1.0', protocolVersion: 'v2', webBuildId: 'test' }
    observeRead(signal => loadTaskRuns(runtime, 't_1', { signal, transport: { call: request => h.registry.read(request) } }), controller.signal, next, failed)
    await vi.waitFor(() => expect(h.connections).toHaveLength(1))
    const runs = h.latest().request.queries[0]
    const initialRuns = { encoded: queryResultBytes(runs, { data: [run('r_1')] }), cursor: cursor(1n, runs.clientQueryId) }
    h.latest().queue.push(...await resultFrames(runs.clientQueryId, initialRuns.encoded, initialRuns.cursor), readyFrame(runs.clientQueryId, initialRuns.cursor))
    await vi.waitFor(() => expect(h.latest().request.queries.some(query => query.query.case === 'getRunLog')).toBe(true))
    const firstLog = h.latest().request.queries.find(query => query.query.case === 'getRunLog')!
    const initialLog = { encoded: queryResultBytes(firstLog, { data: { run_id: 'r_1', content: '日志开始', truncated: false } }), cursor: cursor(1n, firstLog.clientQueryId) }
    h.latest().queue.push(...await resultFrames(firstLog.clientQueryId, initialLog.encoded, initialLog.cursor), readyFrame(firstLog.clientQueryId, initialLog.cursor))
    await vi.waitFor(() => expect(next).toHaveBeenCalledTimes(1))
    h.latest().queue.push(...await resultFrames(firstLog.clientQueryId, queryResultBytes(firstLog, { data: { run_id: 'r_1', content: '日志开始\n新增内容', truncated: false } }), cursor(2n, firstLog.clientQueryId), initialLog))
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith(expect.objectContaining({ log: { run_id: 'r_1', content: '日志开始\n新增内容', truncated: false } })))
    h.latest().queue.push(...await resultFrames(runs.clientQueryId, queryResultBytes(runs, { data: [run('r_2'), run('r_1')] }), cursor(2n, runs.clientQueryId), initialRuns))
    await vi.waitFor(() => expect(h.latest().request.queries.some(query => query.query.case === 'getRunLog' && query.query.value.runId === 'r_2')).toBe(true))
    const secondLog = h.latest().request.queries.find(query => query.query.case === 'getRunLog' && query.query.value.runId === 'r_2')!
    h.latest().queue.push(...await resultFrames(secondLog.clientQueryId, queryResultBytes(secondLog, { data: { run_id: 'r_2', content: '新运行日志', truncated: true } }), cursor(1n, secondLog.clientQueryId)), readyFrame(secondLog.clientQueryId, cursor(1n, secondLog.clientQueryId)))
    await vi.waitFor(() => expect(next).toHaveBeenLastCalledWith(expect.objectContaining({ selectedRunId: 'r_2', log: { run_id: 'r_2', content: '新运行日志', truncated: true } })))
    await vi.waitFor(() => expect(h.latest().request.queries.some(query => query.clientQueryId === firstLog.clientQueryId)).toBe(false))
    expect(h.registry.activeCount).toBe(2)
    for (const [model] of next.mock.calls) expect(model.log.run_id).toBe(model.selectedRunId)
    expect(failed).not.toHaveBeenCalled()
  })
})

test('一次写后确认等待新的 Ready，完成后只释放自己的引用', async () => {
  const h = harness(), reader = h.subscribe();
  await vi.waitFor(() => expect(h.connections).toHaveLength(1));
  const definition = h.latest().request.queries[0];
  const previous = await h.publish(definition, '原结果');
  await vi.waitFor(() => expect(reader.next).toHaveBeenCalledTimes(1));
  const completed = vi.fn();
  const refreshed = refreshRead(signal => h.registry.read({ ...call, signal })).then(completed);
  await vi.waitFor(() => expect(h.connections).toHaveLength(2));
  expect(h.latest().request.queries[0].refresh).toBe(true);
  expect(completed).not.toHaveBeenCalled();
  h.latest().queue.push(readyFrame(definition.clientQueryId, previous.cursor));
  await refreshed;
  expect(completed).toHaveBeenCalledWith(expect.objectContaining({ payload: payload('原结果') }));
  expect(h.registry.activeCount).toBe(1);
  reader.controller.abort();
  expect(h.registry.activeCount).toBe(0);
});
