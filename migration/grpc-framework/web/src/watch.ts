import { ProtocolError, type Cursor, type Frame } from "./model.js"
import { ProjectionStore } from "./reducer.js"
export type WatchInput = { boardId: string; resume: Cursor | undefined; signal: AbortSignal }
export type OpenWatch = (input: WatchInput) => AsyncIterable<Frame>
export type WatchState = "connecting" | "live" | "retrying" | "stopped"
export function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) { resolve(); return }
    const done = (): void => { clearTimeout(timer); signal.removeEventListener("abort", done); resolve() }
    const timer = setTimeout(done, ms)
    signal.addEventListener("abort", done, { once: true })
  })
}
export interface WatchOptions {
  status?: (state: WatchState) => void
  terminal?: (error: unknown) => boolean
  random?: () => number
  delay?: typeof sleep
  livenessMs?: number
}
/** 一次启动对应一个 generation。旧网络回调无法修改重启后的 store。 */
export class WatchSession {
  private generation = 0
  private active: AbortController | undefined
  constructor(private readonly store: ProjectionStore, private readonly open: OpenWatch, private readonly options: WatchOptions = {}) {}
  stop(): void { this.generation++; this.active?.abort(); this.active = undefined; this.store.discardPending() }
  async start(): Promise<void> {
    this.stop()
    const generation = this.generation
    const outer = new AbortController()
    this.active = outer
    const current = (): boolean => this.generation === generation && !outer.signal.aborted
    let failures = 0
    try {
      while (current()) {
        const connection = new AbortController()
        const abort = (): void => connection.abort()
        outer.signal.addEventListener("abort", abort, { once: true })
        let watchdog: ReturnType<typeof setTimeout> | undefined
        const arm = (): void => {
          clearTimeout(watchdog)
          watchdog = setTimeout(abort, this.options.livenessMs ?? 35_000)
        }
        const before = this.store.resume()
        let progressed = false
        this.options.status?.("connecting")
        arm()
        try {
          for await (const frame of this.open({ boardId: this.store.boardId, resume: before, signal: connection.signal })) {
            if (!current() || connection.signal.aborted) break
            this.store.apply(frame)
            arm()
            if (frame.body.kind === "commit" || frame.body.kind === "delta") { progressed = true; this.options.status?.("live") }
            else if (frame.body.kind === "heartbeat" && this.store.resume()) this.options.status?.("live")
          }
        } catch (error) {
          if (!current()) break
          if (this.options.terminal?.(error)) throw error
          if (error instanceof ProtocolError) this.store.requireSnapshot()
        } finally {
          clearTimeout(watchdog)
          outer.signal.removeEventListener("abort", abort)
          connection.abort()
          if (this.generation === generation) this.store.discardPending()
        }
        if (!current()) break
        // 重复接上又立即 EOF 的服务不会用 heartbeat 清空重试计数。
        failures = progressed ? 1 : failures + 1
        if (failures >= 8) throw new Error("订阅连续失败 8 次；需检查宿主或手动重试")
        this.options.status?.("retrying")
        const jitter = 0.8 + 0.4 * (this.options.random?.() ?? Math.random())
        await (this.options.delay ?? sleep)(Math.min(5000, 250 * 2 ** (failures - 1)) * jitter, outer.signal)
      }
    } finally {
      if (this.generation === generation) {
        this.active = undefined
        this.store.discardPending()
        this.options.status?.("stopped")
      }
    }
  }
}
