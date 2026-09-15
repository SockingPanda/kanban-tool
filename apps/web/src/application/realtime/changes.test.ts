import { describe, expect, test, vi } from "vitest"
import { ChangeProtocolError, ChangeSequence, createChangeRealtime, type ChangeFrame, type OpenChanges } from "./changes"
import type { BoardRealtimeContext } from "./source"

const frame = (sequence = 1n, reason: "attached" | "write_hint" = "attached", overrides: Partial<ChangeFrame> = {}): ChangeFrame =>
  ({ boardId: "b_one", epoch: "epoch", sequence, kind: "refresh", reason, ...overrides })
const heartbeat = (sequence = 1n): ChangeFrame => ({ boardId: "b_one", epoch: "epoch", sequence, kind: "heartbeat" })
const context = (overrides: Partial<BoardRealtimeContext> = {}): BoardRealtimeContext =>
  ({ boardId: "b_one", boardSelector: "one", onRefresh: vi.fn(), onState: vi.fn(), ...overrides })
const tick = (): Promise<void> => new Promise(resolve => setImmediate(resolve))

describe("正式刷新流的连接顺序与生命周期", () => {
  test("attached 和连续写提示使查询失效，heartbeat 保持同一序号", () => {
    const sequence = new ChangeSequence("b_one")
    expect(sequence.accept(frame())).toBe(true)
    expect(sequence.accept(heartbeat())).toBe(false)
    expect(sequence.accept(frame(2n, "write_hint"))).toBe(true)
    expect(sequence.accept(heartbeat(2n))).toBe(false)
  })

  test.each([
    heartbeat(), frame(1n, "write_hint"), frame(2n), frame(0n), frame(18446744073709551616n),
    frame(1n, "attached", { boardId: "b_other" }), frame(1n, "attached", { epoch: "" }),
    frame(1n, "attached", { epoch: "x".repeat(129) }),
  ])("拒绝无效的初始 frame %#", invalid => {
    expect(() => new ChangeSequence("b_one").accept(invalid)).toThrow(ChangeProtocolError)
  })

  test.each([
    frame(), frame(3n, "write_hint"), frame(2n, "write_hint", { epoch: "other" }),
    frame(2n, "write_hint", { boardId: "b_other" }), heartbeat(2n), { ...heartbeat(), reason: "write_hint" as const },
  ])("拒绝重复、缺口和跨连接 frame %#", invalid => {
    const sequence = new ChangeSequence("b_one")
    sequence.accept(frame())
    expect(() => sequence.accept(invalid)).toThrow(ChangeProtocolError)
  })

  test("每次 RPC 重连以新 attached 刷新，连续 EOF 不被 attached 或 heartbeat 清零", async () => {
    const refresh = vi.fn()
    let connection = 0
    const open = vi.fn<OpenChanges>(async function* (boardId, signal) {
      expect(boardId).toBe("b_one")
      expect(signal.aborted).toBe(false)
      const epoch = String(++connection)
      yield frame(1n, "attached", { epoch })
      yield { ...heartbeat(), epoch }
    })
    const controller = createChangeRealtime("rpc", open, { maxFailures: 3, delay: async () => undefined }).create(context({ onRefresh: refresh }))
    controller.start()
    controller.start()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("failed"))
    expect(open).toHaveBeenCalledTimes(3)
    expect(refresh).toHaveBeenCalledTimes(3)
    expect(open.mock.calls.every(([, signal]) => signal.aborted)).toBe(true)
    controller.stop()
  })

  test("手动重试和释放后忽略不遵守 abort 的迟到 frame", async () => {
    const resolve: Array<(value: IteratorResult<ChangeFrame>) => void> = []
    const signals: AbortSignal[] = []
    const returned = vi.fn(async (): Promise<IteratorResult<ChangeFrame>> => ({ done: true, value: undefined }))
    const open: OpenChanges = (_boardId, signal) => {
      signals.push(signal)
      return { [Symbol.asyncIterator]: () => ({ next: () => new Promise(done => resolve.push(done)), return: returned }) }
    }
    const refresh = vi.fn()
    const controller = createChangeRealtime("rpc", open).create(context({ onRefresh: refresh }))
    controller.start()
    await vi.waitFor(() => expect(resolve).toHaveLength(1))
    controller.retry()
    await vi.waitFor(() => expect(resolve).toHaveLength(2))
    resolve[0]?.({ done: false, value: frame() })
    controller.stop()
    resolve[1]?.({ done: false, value: frame() })
    await tick()
    expect(refresh).not.toHaveBeenCalled()
    expect(signals.every(signal => signal.aborted)).toBe(true)
    expect(returned).toHaveBeenCalledTimes(2)
  })

  test("看板的独立 controller 不共享 epoch、序号或取消", async () => {
    const releases: Array<() => void> = []
    const open: OpenChanges = async function* (boardId, signal) {
      yield frame(1n, "attached", { boardId, epoch: boardId })
      await new Promise<void>(resolve => { releases.push(resolve); signal.addEventListener("abort", () => resolve(), { once: true }) })
    }
    const source = createChangeRealtime("rpc", open)
    const firstRefresh = vi.fn(), secondRefresh = vi.fn()
    const first = source.create(context({ onRefresh: firstRefresh }))
    const second = source.create(context({ boardId: "b_two", boardSelector: "two", onRefresh: secondRefresh }))
    first.start(); second.start()
    await vi.waitFor(() => expect(secondRefresh).toHaveBeenCalledOnce())
    first.stop()
    expect(firstRefresh).toHaveBeenCalledOnce()
    expect(second.snapshot().state).toBe("live")
    second.stop()
    releases.forEach(release => release())
  })

  test("liveness 超时关闭无响应 iterator 并按预算重试", async () => {
    const open = vi.fn<OpenChanges>(() => ({ [Symbol.asyncIterator]: () => ({
      next: () => new Promise(() => undefined), return: () => new Promise(() => undefined),
    }) }))
    const controller = createChangeRealtime("rpc", open, { livenessMs: 5, maxFailures: 2, delay: async () => undefined }).create(context())
    controller.start()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("failed"))
    expect(open).toHaveBeenCalledTimes(2)
    controller.stop()
  })

  test("刷新观察者中 stop 不再发布 live，重试等待中的 stop 不再发起 RPC", async () => {
    const states = vi.fn()
    const controller = createChangeRealtime("rpc", async function* () { yield frame() }).create(context({
      onRefresh: () => controller.stop(), onState: states,
    }))
    controller.start()
    await vi.waitFor(() => expect(controller.snapshot().state).toBe("stopped"))
    expect(states.mock.calls.some(([state]) => state === "live")).toBe(false)
    const open = vi.fn<OpenChanges>(() => ({ [Symbol.asyncIterator]: () => ({ next: async () => ({ done: true, value: undefined }) }) }))
    const delay = vi.fn(async (_ms: number, signal: AbortSignal) => { await new Promise<void>(resolve => signal.addEventListener("abort", () => resolve(), { once: true })) })
    const retrying = createChangeRealtime("rpc", open, { delay }).create(context())
    retrying.start()
    await vi.waitFor(() => expect(delay).toHaveBeenCalledOnce())
    retrying.stop()
    await tick()
    expect(open).toHaveBeenCalledOnce()
  })
})
