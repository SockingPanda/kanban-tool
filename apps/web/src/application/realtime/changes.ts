import type { BoardRealtimeContext, BoardRealtimeController, BoardRealtimeSource, RealtimeState } from "./source"

export type ChangeFrame = Readonly<{
  boardId: string
  epoch: string
  sequence: bigint
  kind: "refresh" | "heartbeat"
  reason?: "attached" | "write_hint"
}>
export type OpenChanges = (boardId: string, signal: AbortSignal) => AsyncIterable<ChangeFrame>
export interface ChangeOptions {
  readonly livenessMs?: number
  readonly maxFailures?: number
  readonly delay?: typeof sleep
  readonly random?: () => number
  readonly terminal?: (error: unknown) => boolean
}
export class ChangeProtocolError extends Error {}

/** 只校验当前连接的顺序；sequence 绝不能作为历史恢复 cursor 使用。 */
export class ChangeSequence {
  private epoch: string | undefined
  private sequence = 0n
  constructor(private readonly boardId: string) {}
  accept(frame: ChangeFrame): boolean {
    if (frame.boardId !== this.boardId || !frame.epoch || frame.epoch.length > 128
      || typeof frame.sequence !== "bigint" || frame.sequence < 1n || frame.sequence > 18446744073709551615n) {
      throw new ChangeProtocolError("刷新流身份或序号无效")
    }
    if (this.epoch === undefined) {
      if (frame.kind !== "refresh" || frame.reason !== "attached" || frame.sequence !== 1n) {
        throw new ChangeProtocolError("刷新流必须从 attached 开始")
      }
      this.epoch = frame.epoch
      this.sequence = frame.sequence
      return true
    }
    if (frame.epoch !== this.epoch) throw new ChangeProtocolError("同一流的 epoch 改变")
    if (frame.kind === "heartbeat") {
      if (frame.sequence !== this.sequence || frame.reason !== undefined) throw new ChangeProtocolError("心跳序号无效")
      return false
    }
    if (frame.kind !== "refresh" || frame.reason !== "write_hint" || frame.sequence !== this.sequence + 1n) {
      throw new ChangeProtocolError("刷新序号缺口或重复")
    }
    this.sequence = frame.sequence
    return true
  }
}
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) { reject(abortError()); return }
    const done = (): void => { signal.removeEventListener("abort", abort); resolve() }
    const timer = setTimeout(done, ms)
    const abort = (): void => { clearTimeout(timer); reject(abortError()) }
    signal.addEventListener("abort", abort, { once: true })
  })
}
function abortError(): Error { return Object.assign(new Error("订阅已取消"), { name: "AbortError" }) }
/** 取消优先返回，防止未遵守 signal 的 iterator 把会话生命周期卡住。 */
function nextFrame(iterator: AsyncIterator<ChangeFrame>, signal: AbortSignal): Promise<IteratorResult<ChangeFrame>> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) { reject(abortError()); return }
    const aborted = (): void => reject(abortError())
    signal.addEventListener("abort", aborted, { once: true })
    Promise.resolve().then(() => {
      if (signal.aborted) throw abortError()
      return iterator.next()
    }).then(resolve, reject).finally(() => signal.removeEventListener("abort", aborted)).catch(() => undefined)
  })
}
class ChangeController implements BoardRealtimeController {
  private generation = 0
  private active: AbortController | undefined
  private state: RealtimeState = "stopped"
  constructor(private readonly context: BoardRealtimeContext, private readonly open: OpenChanges, private readonly options: ChangeOptions) {}
  snapshot(): { readonly state: RealtimeState } { return { state: this.state } }
  private report(state: RealtimeState, error?: unknown): void {
    this.state = state
    try { this.context.onState(state, error) } catch { /* 观察者不影响订阅取消。 */ }
  }
  start(): void {
    if (this.active) return
    const active = new AbortController()
    const generation = ++this.generation
    this.active = active
    void this.run(active, generation).catch(error => {
      if (generation === this.generation && !active.signal.aborted) { this.active = undefined; this.report("failed", error) }
    })
  }
  stop(): void {
    ++this.generation
    this.active?.abort()
    this.active = undefined
    this.report("stopped")
  }
  retry(): void { this.stop(); this.start() }
  private async run(outer: AbortController, generation: number): Promise<void> {
    const current = (): boolean => generation === this.generation && !outer.signal.aborted
    let failures = 0
    while (current()) {
      const connection = new AbortController()
      const abort = (): void => connection.abort()
      outer.signal.addEventListener("abort", abort, { once: true })
      let watchdog: ReturnType<typeof setTimeout> | undefined
      const arm = (): void => { clearTimeout(watchdog); watchdog = setTimeout(abort, this.options.livenessMs ?? 35000) }
      let iterator: AsyncIterator<ChangeFrame> | undefined
      let progressed = false
      let failure: unknown = new Error("刷新流提前关闭")
      this.report("connecting")
      arm()
      try {
        if (!current()) break
        iterator = this.open(this.context.boardId, connection.signal)[Symbol.asyncIterator]()
        const sequence = new ChangeSequence(this.context.boardId)
        while (current()) {
          const next = await nextFrame(iterator, connection.signal)
          if (!current() || connection.signal.aborted || next.done) break
          const refresh = sequence.accept(next.value)
          arm()
          if (refresh) this.context.onRefresh()
          if (!current()) break
          // attached / 单个 heartbeat 不消除连续 EOF 的失败预算。
          if (next.value.kind === "refresh" && next.value.reason === "write_hint") progressed = true
          this.report("live")
        }
      } catch (error) {
        failure = error
        if (current() && this.options.terminal?.(error)) throw error
      } finally {
        clearTimeout(watchdog)
        outer.signal.removeEventListener("abort", abort)
        connection.abort()
        // return 也可能不遵守取消；不等待它，不允许迟到结果触碰当前 generation。
        if (iterator?.return) { try { void Promise.resolve(iterator.return()).catch(() => undefined) } catch { /* 取消后的清理错误不重启会话。 */ } }
      }
      if (!current()) break
      failures = progressed ? 1 : failures + 1
      if (failures >= (this.options.maxFailures ?? 8)) throw failure
      this.report("retrying", failure)
      const random = Math.min(1, Math.max(0, this.options.random?.() ?? Math.random()))
      await (this.options.delay ?? sleep)(Math.min(5000, 250 * 2 ** (failures - 1)) * (0.8 + random * 0.4), outer.signal)
    }
    if (generation === this.generation) { this.active = undefined; this.report("stopped") }
  }
}
export function createChangeRealtime(key: string, open: OpenChanges, options: ChangeOptions = {}): BoardRealtimeSource {
  if (!key.trim()) throw new Error("实时数据源 key 不能为空")
  if (!Number.isFinite(options.livenessMs ?? 35000) || (options.livenessMs ?? 35000) < 1) throw new Error("livenessMs 无效")
  if (!Number.isInteger(options.maxFailures ?? 8) || (options.maxFailures ?? 8) < 1) throw new Error("maxFailures 无效")
  return {
    key,
    create(context) {
      const invalidCharacter = [...context.boardId].some(character => {
        const point = character.codePointAt(0) ?? 0
        return /\s/.test(character) || point <= 0x1f || point === 0x7f
      })
      if (!context.boardId.startsWith("b_") || context.boardId.length <= 2 || invalidCharacter || !context.boardSelector.trim()) throw new Error("看板身份无效")
      return new ChangeController(context, open, options)
    },
  }
}
