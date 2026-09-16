import { create } from '@bufbuild/protobuf'
import { QueryDefinitionSchema, WatchQueriesRequestSchema, type QueryFrame, type WatchQueriesRequest } from '../../generated/rpc/kanban/v1/query_pb'
import type { ReadDependency } from '../../application/query/observe-read'
import { readObservedDependency } from '../../application/query/observe-read'
import { RpcTransportError, type RpcTransportResponse } from '../../application/data/rpc-transport'
import { encodeQueryDefinition, queryIdentity, type QueryCall } from '../../lib/rpc/query-codec'
import { QueryProjection, sameQueryCursor } from '../../lib/rpc/query-projection'
import { queryFailureError, rpcTransportError } from '../../lib/rpc/errors'

export const MAX_ACTIVE_QUERIES = 64
export const MAX_REGISTRY_BYTES = 256 * 1024 * 1024
export type QueryWatch = (request: WatchQueriesRequest, signal: AbortSignal) => AsyncIterable<QueryFrame>
export type QueryConnectionState = 'connecting' | 'live' | 'offline'

interface Entry {
  readonly key: string
  readonly id: string
  readonly projection: QueryProjection
  readonly listeners: Set<() => void>
  readonly waiters: Set<() => void>
  version: number
  error: Error | null
  fresh: boolean
  refreshing: boolean
  refreshVersion: number
}

/** 同一 data source 只有一个网络连接；registry 是所有 query 数据与 cursor 的唯一 owner。 */
export class QueryRegistry {
  private readonly entries = new Map<string, Entry>()
  private readonly states = new Set<(state: QueryConnectionState) => void>()
  private connection: AbortController | null = null
  private generation = 0
  private sequence = 0
  private scheduled = false
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private backoff = 250
  private state: QueryConnectionState = 'connecting'
  private suspended = false
  private observingPage = false
  private readonly pageHide = () => {
    this.suspended = true
    this.generation += 1
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    this.reconnectTimer = null
    this.connection?.abort()
    this.connection = null
    for (const entry of this.entries.values()) entry.projection.discard()
  }
  private readonly pageShow = () => { if (this.suspended) { this.suspended = false; this.schedule() } }

  constructor(private readonly watch: QueryWatch, private readonly pageLifecycle?: EventTarget) {}

  get activeCount(): number { return this.entries.size }

  subscribeState(listener: (state: QueryConnectionState) => void): () => void {
    this.states.add(listener)
    listener(this.state)
    return () => this.states.delete(listener)
  }

  retry(): void {
    for (const entry of this.entries.values()) this.refresh(entry)
    this.schedule()
  }

  read(call: QueryCall): Promise<RpcTransportResponse> {
    if (call.signal?.aborted) return Promise.reject(call.signal.reason ?? new DOMException('读取已取消。', 'AbortError'))
    const key = queryIdentity(call)
    let entry = this.entries.get(key)
    if (!entry) {
      if (this.entries.size >= MAX_ACTIVE_QUERIES) return Promise.reject(new RpcTransportError('response_too_large', '活跃查询超过 64 个预算。'))
      const id = `q${++this.sequence}`
      entry = { key, id, projection: new QueryProjection(encodeQueryDefinition(call, id)), listeners: new Set(), waiters: new Set(), version: 0, error: null, fresh: true, refreshing: false, refreshVersion: 0 }
      this.entries.set(key, entry)
    }
    const current = entry
    const dependency: ReadDependency<RpcTransportResponse> = {
      key,
      version: () => current.version,
      read: () => this.value(current, call.signal),
      subscribe: (changed) => {
        const first = current.listeners.size === 0
        current.listeners.add(changed)
        if (first) { this.observePage(); this.schedule() }
        let released = false
        return () => {
          if (released) return
          released = true
          current.listeners.delete(changed)
          if (current.listeners.size === 0 && this.entries.get(key) === current) {
            this.entries.delete(key)
            current.projection.discard()
            current.error = new DOMException('查询已释放。', 'AbortError')
            this.notify(current)
            if (this.entries.size === 0) this.releasePage()
            this.schedule()
          }
        }
      },
      retry: () => { this.refresh(current); this.schedule() },
    }
    return readObservedDependency(call.signal, dependency)
  }

  private refresh(entry: Entry): void {
    entry.refreshing = true
    entry.refreshVersion += 1
    entry.error = null
  }

  private value(entry: Entry, signal: AbortSignal | undefined): Promise<{ readonly value: RpcTransportResponse; readonly version: number }> {
    return new Promise((resolve, reject) => {
      const finish = () => {
        if (signal?.aborted) { cleanup(); reject(signal.reason ?? new DOMException('读取已取消。', 'AbortError')); return }
        if (entry.error) { cleanup(); reject(entry.error); return }
        if (entry.projection.committed && !entry.refreshing) {
          cleanup(); resolve({ value: entry.projection.committed.response, version: entry.version })
        }
      }
      const cleanup = () => { entry.waiters.delete(finish); signal?.removeEventListener('abort', finish) }
      entry.waiters.add(finish)
      signal?.addEventListener('abort', finish, { once: true })
      finish()
    })
  }

  private notify(entry: Entry): void {
    for (const waiter of [...entry.waiters]) waiter()
    for (const listener of [...entry.listeners]) listener()
  }

  private setState(state: QueryConnectionState): void {
    if (state === this.state) return
    this.state = state
    for (const listener of this.states) listener(state)
  }

  /** document 离开不触发 React cleanup；必须阻止浏览器取消请求后又在旧页面重连。 */
  private observePage(): void {
    if (this.observingPage || !this.pageLifecycle) return
    this.observingPage = true
    this.pageLifecycle.addEventListener('pagehide', this.pageHide)
    this.pageLifecycle.addEventListener('pageshow', this.pageShow)
  }

  private releasePage(): void {
    this.pageLifecycle?.removeEventListener('pagehide', this.pageHide)
    this.pageLifecycle?.removeEventListener('pageshow', this.pageShow)
    this.observingPage = false
    this.suspended = false
  }

  private schedule(): void {
    if (this.scheduled) return
    this.scheduled = true
    queueMicrotask(() => { this.scheduled = false; this.connect() })
  }

  private reconnectLater(): void {
    if (this.reconnectTimer) return
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      if ([...this.entries.values()].some(entry => entry.error !== null)) this.connect()
    }, this.backoff)
    this.backoff = Math.min(this.backoff * 2, 10_000)
  }

  private connect(): void {
    if (this.suspended) return
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    this.reconnectTimer = null
    this.generation += 1
    const generation = this.generation
    this.connection?.abort()
    for (const entry of this.entries.values()) entry.projection.discard()
    this.connection = null
    if (this.entries.size === 0) return
    const controller = new AbortController()
    this.connection = controller
    const active = new Map([...this.entries.values()].map(entry => [entry.id, entry]))
    const refreshVersions = new Map([...active.values()].map(entry => [entry.id, entry.refreshVersion]))
    const request = create(WatchQueriesRequestSchema, {
      protocolVersion: 1,
      queries: [...active.values()].map(entry => create(QueryDefinitionSchema, {
        ...entry.projection.definition,
        resume: entry.fresh ? undefined : entry.projection.committed?.cursor,
        refresh: entry.refreshing,
      })),
    })
    this.setState('connecting')
    const current = () => !controller.signal.aborted && generation === this.generation
    const run = async () => {
      try {
        for await (const frame of this.watch(request, controller.signal)) {
          if (!current()) return
          const body = frame.body
          if (body.case === 'heartbeat' && frame.clientQueryId === '') continue
          const entry = active.get(frame.clientQueryId)
          if (!entry) throw new RpcTransportError('invalid_bytes', '查询帧不属于当前连接。')
          switch (body.case) {
          case 'ready': {
            if (!sameQueryCursor(entry.projection.committed?.cursor, body.value.cursor)) {
              throw new RpcTransportError('invalid_bytes', '查询 Ready 不匹配已提交结果。')
            }
            // refresh 立即增加代次；旧流已排队的 Ready 不能确认下一条连接才会发送的请求。
            if (entry.refreshing && refreshVersions.get(entry.id) !== entry.refreshVersion) continue
            if (entry.refreshing || entry.error) {
              entry.refreshing = false
              entry.error = null
              entry.version += 1
              this.notify(entry)
            }
            this.setState('live')
            continue
          }
          case 'failure': {
            entry.projection.discard()
            entry.error = queryFailureError(body.value.error)
            entry.version += 1
            this.notify(entry)
            if (body.value.retryable) { entry.refreshing = true; this.reconnectLater() }
            continue
          }
          case 'begin': {
            const reserved = [...this.entries.values()].reduce((sum, item) => sum + (item.projection.committed?.encoded.byteLength ?? 0) + item.projection.pendingBytes, 0)
            const begin = body.value
            if (reserved + Number(begin.patchSize) + Number(begin.resultSize) > MAX_REGISTRY_BYTES) {
              throw new RpcTransportError('response_too_large', '查询暂存与结果超过 256 MiB 总预算。')
            }
            break
          }
          default: break
          }
          const committed = await entry.projection.accept(frame)
          if (!current()) return
          const bytes = [...this.entries.values()].reduce((sum, item) => sum + (item.projection.committed?.encoded.byteLength ?? 0) + item.projection.pendingBytes, 0)
          if (bytes > MAX_REGISTRY_BYTES) throw new RpcTransportError('response_too_large', '查询结果总量超过 256 MiB 预算。')
          if (committed) {
            entry.version += 1
            entry.error = null
            entry.fresh = false
            this.backoff = 250
            this.notify(entry)
          }
          this.setState('live')
        }
        if (current()) throw new RpcTransportError('offline', '查询连接已结束，正在重连。')
      } catch (cause) {
        if (!current()) return
        const error = cause instanceof RpcTransportError ? cause : rpcTransportError(cause)
        for (const entry of active.values()) {
          entry.projection.discard()
          if (error.kind === 'invalid_bytes' || error.kind === 'invalid_json') entry.fresh = true
          entry.error = error
          entry.version += 1
          this.notify(entry)
        }
        this.setState('offline')
        if (error.kind !== 'response_too_large') this.reconnectLater()
      } finally { controller.abort() }
    }
    void run()
  }
}
