import { afterEach, describe, expect, test, vi } from 'vitest'
import { createHostDataSource } from '../../adapters/host/data-source'
import type { BoardReadModel, BoardReadQuery } from '../data/board-read-model'
import { asCanonicalBoardId } from '../../domain/board-id'
import type { WebRuntimeConfig } from '../../lib/runtime'
import type { BoardViewModel } from '../../domain/tasks/board'
import { acquireBoardSession, activeBoardSessionCount, bindBoardResourceIdentity, boardSessionRevision, hasActiveBoardSession, reconnectActiveBoardSession, resourceIdentityKey, resetBoardSessionsForTests, runtimeIdentityKey, subscribeBoardSessions, type BoardReadResource } from './board-session-registry'

const runtime: WebRuntimeConfig = { apiBaseUrl: '', webBasePath: '/app/', actor: 'test', defaultBoard: 'default', serverVersion: '3.0.0', protocolVersion: 'v2', webBuildId: 'test' }
const model: BoardViewModel = { board: { id: 'b_default', slug: 'default', name: 'Default' }, columns: [], tasksByStatus: {} }
const readModel: BoardReadModel = { identity: { selector: 'default', canonicalBoardId: asCanonicalBoardId('b_default'), slug: 'default', name: 'Default' }, columns: [], tasksByStatus: {} }

function controlledQuery() {
  let next!: (model: BoardReadModel) => void
  let failed!: (error: unknown) => void
  let state!: (state: 'connecting' | 'live' | 'offline') => void
  let signal!: AbortSignal
  const releaseConnection = vi.fn()
  const query = {
    load: vi.fn(async () => readModel), reload: vi.fn<BoardReadQuery['reload']>(async () => readModel), invalidate: vi.fn(),
    observe: vi.fn<BoardReadQuery['observe']>((abort, onNext, onError) => { signal = abort; next = onNext; failed = onError }),
    subscribeConnection: vi.fn<BoardReadQuery['subscribeConnection']>(listener => { state = listener; return releaseConnection }),
  } satisfies BoardReadQuery
  return { query, next: (value = readModel) => next(value), fail: () => failed(new Error('断线')), state: (value: 'connecting' | 'live' | 'offline') => state(value), get signal() { return signal }, releaseConnection }
}
function resource(query: BoardReadQuery, config = runtime, selector = 'default', id = 'b_default', slug = 'default'): BoardReadResource {
  const boardId = asCanonicalBoardId(id)
  return { selector, query, runtimeKey: runtimeIdentityKey(config), identityKey: resourceIdentityKey(config, selector, boardId), canonicalBoardId: boardId, resolvedSlug: slug, sessionGeneration: 0 }
}
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((ok, fail) => { resolve = ok; reject = fail })
  return { promise, resolve, reject }
}
afterEach(resetBoardSessionsForTests)

describe('完整查询的 canonical 看板会话', () => {
  test('Host 仅提供业务操作与完整查询，不向页面暴露 transport', () => {
    const source = createHostDataSource(runtime, { documentBaseURI: 'http://127.0.0.1/app/' })
    expect(source.createBoardReadQuery(runtime, 'default')).toHaveProperty('observe')
    expect(source).not.toHaveProperty('transport')
  })

  test('同 canonical ID 的 slug/ID selector 共用一个 observer，最后卸载才释放', () => {
    const h = controlledQuery(), other = controlledQuery(), publish = vi.fn()
    const a = acquireBoardSession(runtime, model, resource(h.query), publish, vi.fn())
    const b = acquireBoardSession(runtime, model, resource(other.query, runtime, 'b_default'), publish, vi.fn())
    expect(activeBoardSessionCount()).toBe(1)
    expect(h.query.observe).toHaveBeenCalledOnce()
    expect(other.query.observe).not.toHaveBeenCalled()
    h.next()
    expect(publish).toHaveBeenCalledTimes(2)
    a.release(); a.release()
    expect(h.signal.aborted).toBe(false)
    publish.mockClear(); h.next()
    expect(publish).toHaveBeenCalledOnce()
    b.release()
    expect(h.signal.aborted).toBe(true)
    expect(h.releaseConnection).toHaveBeenCalledOnce()
    expect(h.query.invalidate).toHaveBeenCalledOnce()
    expect(other.query.invalidate).not.toHaveBeenCalled()
    expect(activeBoardSessionCount()).toBe(0)
  })

  test('请求期间释放一个 owner，完整结果仍交给另一个挂载者', async () => {
    const h = controlledQuery(), pending = deferred<BoardReadModel>()
    h.query.reload.mockReturnValueOnce(pending.promise)
    const first = vi.fn(), retained = vi.fn()
    const a = acquireBoardSession(runtime, model, resource(h.query), first, vi.fn())
    const b = acquireBoardSession(runtime, model, resource(h.query), retained, vi.fn())
    const wait = a.refresh(); a.release()
    expect(h.signal.aborted).toBe(false)
    pending.resolve(readModel); await wait
    expect(first).not.toHaveBeenCalled()
    expect(retained).toHaveBeenCalledWith(readModel)
    b.release()
  })

  test('同 board 重新挂载后丢弃旧 observer、旧状态和迟到刷新', async () => {
    const old = controlledQuery(), current = controlledQuery(), pending = deferred<BoardReadModel>()
    old.query.reload.mockReturnValueOnce(pending.promise)
    const oldModel = vi.fn(), nextModel = vi.fn(), nextState = vi.fn()
    const a = acquireBoardSession(runtime, model, resource(old.query), oldModel, vi.fn())
    const wait = a.refresh(); a.release()
    const b = acquireBoardSession(runtime, model, resource(current.query), nextModel, nextState)
    old.next(); old.state('live'); old.fail(); pending.resolve(readModel); await wait
    a.retry(); a.reconnect(); a.release(); await a.refresh()
    expect(oldModel).not.toHaveBeenCalled()
    expect(nextModel).not.toHaveBeenCalled()
    expect(nextState).not.toHaveBeenCalled()
    expect(current.query.reload).not.toHaveBeenCalled()
    current.next(); expect(nextModel).toHaveBeenCalledOnce()
    b.release()
  })

  test('不同 board 与 runtime/build 各自隔离', () => {
    const a = controlledQuery(), b = controlledQuery(), c = controlledQuery()
    const alternate = { ...runtime, webBuildId: 'replacement' }
    const otherModel = { ...model, board: { id: 'b_other', slug: 'other', name: 'Other' } }
    const first = vi.fn(), second = vi.fn(), third = vi.fn()
    acquireBoardSession(runtime, model, resource(a.query), first, vi.fn())
    acquireBoardSession(runtime, otherModel, resource(b.query, runtime, 'other', 'b_other', 'other'), second, vi.fn())
    acquireBoardSession(alternate, model, resource(c.query, alternate), third, vi.fn())
    expect(activeBoardSessionCount()).toBe(3)
    a.next(); expect(first).toHaveBeenCalledOnce(); expect(second).not.toHaveBeenCalled(); expect(third).not.toHaveBeenCalled()
    expect(hasActiveBoardSession(runtime, 'other')).toBe(true)
    expect(hasActiveBoardSession({ ...runtime, apiBaseUrl: '/new' })).toBe(false)
  })

  test('Ready 确认前不会完成显式刷新或重建 observer', async () => {
    const h = controlledQuery(), pending = deferred<BoardReadModel>(), publish = vi.fn(), completed = vi.fn()
    h.query.reload.mockReturnValueOnce(pending.promise)
    const handle = acquireBoardSession(runtime, model, resource(h.query), publish, vi.fn())
    const wait = handle.refresh().then(completed)
    await Promise.resolve()
    expect(publish).not.toHaveBeenCalled(); expect(completed).not.toHaveBeenCalled()
    pending.resolve(readModel); await wait
    expect(publish).toHaveBeenCalledWith(readModel)
    expect(completed).toHaveBeenCalledOnce()
    expect(h.query.observe).toHaveBeenCalledOnce()
  })

  test('每次显式刷新请求都有独立 Ready 等待，后到写入不借用前次完成', async () => {
    const h = controlledQuery(), first = deferred<BoardReadModel>(), second = deferred<BoardReadModel>(), done = vi.fn()
    h.query.reload.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const handle = acquireBoardSession(runtime, model, resource(h.query), vi.fn(), vi.fn())
    const a = handle.refresh(), b = handle.refresh().then(done)
    first.resolve(readModel); await a
    expect(done).not.toHaveBeenCalled()
    second.resolve(readModel); await b
    expect(done).toHaveBeenCalledOnce()
    expect(h.signal.aborted).toBe(false)
  })

  test('非 live 可以显式重连，live 保持原连接；故障直接报告 stale', async () => {
    const h = controlledQuery(), state = vi.fn()
    acquireBoardSession(runtime, model, resource(h.query), vi.fn(), state)
    expect(reconnectActiveBoardSession(runtime, 'missing')).toBe('unavailable')
    expect(reconnectActiveBoardSession(runtime, 'default')).toBe('reconnecting')
    expect(h.query.reload).toHaveBeenCalledOnce()
    h.state('live'); expect(reconnectActiveBoardSession(runtime)).toBe('already-live')
    h.state('offline'); expect(state).toHaveBeenLastCalledWith('stale')
    h.state('connecting'); expect(state).toHaveBeenLastCalledWith('connecting')
    h.fail(); expect(state).toHaveBeenLastCalledWith('stale')
    await Promise.resolve()
  })

  test('同步 observer 构造失败释放资源，后续同 key 仍可订阅', () => {
    const h = controlledQuery()
    h.query.observe.mockImplementationOnce(() => { throw new Error('订阅失败') })
    expect(() => acquireBoardSession(runtime, model, resource(h.query), vi.fn(), vi.fn())).toThrow('订阅失败')
    expect(activeBoardSessionCount()).toBe(0)
    expect(h.query.invalidate).toHaveBeenCalledOnce()
    acquireBoardSession(runtime, model, resource(h.query), vi.fn(), vi.fn())
    expect(activeBoardSessionCount()).toBe(1)
  })

  test('resource 的 runtime、selector 与 canonical identity 必须共同匹配', () => {
    const h = controlledQuery(), value = resource(h.query)
    expect(bindBoardResourceIdentity(runtime, value, readModel)).toBe(value.identityKey)
    expect(() => bindBoardResourceIdentity({ ...runtime, webBuildId: 'new' }, value, readModel)).toThrow('runtime')
    expect(() => bindBoardResourceIdentity(runtime, value, { ...readModel, identity: { ...readModel.identity, selector: 'other' } })).toThrow('selector')
    expect(() => acquireBoardSession(runtime, model, { ...value, canonicalBoardId: asCanonicalBoardId('b_other') }, vi.fn(), vi.fn())).toThrow('identity')
    expect(activeBoardSessionCount()).toBe(0)
  })

  test('reset 后旧 handle 无法释放或重试同 key 新会话', () => {
    const first = controlledQuery(), next = controlledQuery(), publish = vi.fn()
    const old = acquireBoardSession(runtime, model, resource(first.query), vi.fn(), vi.fn())
    resetBoardSessionsForTests()
    acquireBoardSession(runtime, model, resource(next.query), publish, vi.fn())
    old.release(); old.retry(); old.reconnect(); first.next()
    expect(activeBoardSessionCount()).toBe(1)
    expect(next.query.reload).not.toHaveBeenCalled()
    next.next(); expect(publish).toHaveBeenCalledOnce()
  })

  test('会话归属通知只在 acquire/release 改变，不把 query 内容更新转换成全局刷新', () => {
    const h = controlledQuery(), listener = vi.fn(), before = boardSessionRevision()
    const unsubscribe = subscribeBoardSessions(listener)
    const handle = acquireBoardSession(runtime, model, resource(h.query), vi.fn(), vi.fn())
    expect(boardSessionRevision()).toBeGreaterThan(before)
    h.next(); h.state('live')
    expect(listener).toHaveBeenCalledOnce()
    handle.release(); expect(listener).toHaveBeenCalledTimes(2)
    unsubscribe()
  })
})
