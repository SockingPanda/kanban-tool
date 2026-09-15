import { describe, expect, test, vi } from "vitest"
import { createHostRealtime } from "../../adapters/host/realtime"

import { asCanonicalBoardId, type SyncTelemetryEntry } from "../sync/contracts"
import { classifyExplorerSessionTelemetry, coalesceExplorerBoundary } from "../workspace/session-events"
import { boardSyncStatusForTelemetry } from "../workspace/board-live-state"
import { bindBoardRealtime } from "./bind"
import type { BoardRealtimeContext, BoardRealtimeSource, RealtimeState } from "./source"

const boardId = asCanonicalBoardId("b_generated")

function binding() {
  let context: BoardRealtimeContext | undefined
  let state: RealtimeState = "stopped"
  const start = vi.fn(() => { state = "connecting" })
  const stop = vi.fn(() => { state = "stopped" })
  const retry = vi.fn(() => { state = "connecting" })
  const source: BoardRealtimeSource = {
    key: "grpc-test",
    create(value) {
      context = value
      return { start, stop, retry, snapshot: () => ({ state }) }
    },
  }
  const record = vi.fn<(entry: SyncTelemetryEntry) => void>()
  const bound = bindBoardRealtime(source, { boardId, boardSelector: "generated", record })
  return { bound, record, start, stop, retry, context: () => context!, setState: (value: RealtimeState) => { state = value } }
}

describe("Atlas 实时控制消息与实际 application 接口", () => {
  test("刷新提示沿已有 Explorer 边界合并，不构造审计事件", () => {
    const value = binding()
    value.bound.start()
    value.context().onRefresh()
    value.context().onRefresh()
    expect(value.record).toHaveBeenCalledWith({
      type: "rpc-refresh-required", boardId, cursor: 0,
      details: { controlOnly: true, realtimeSource: "grpc-test" },
    })
    expect(coalesceExplorerBoundary(value.record.mock.calls.map(([entry]) => entry.type))).toEqual({
      invalidationDelta: 1, eventsRefreshDelta: 1,
    })
    // 模拟 App 的实际入口分类；旧重复 allowlist 会在这个位置丢掉 RPC 提示。
    const pageBoundaries = value.record.mock.calls.flatMap(([entry]) => classifyExplorerSessionTelemetry(entry.type) === "boundary" ? [entry.type] : [])
    expect(pageBoundaries).toHaveLength(2)
    expect(coalesceExplorerBoundary(pageBoundaries)).toEqual({ invalidationDelta: 1, eventsRefreshDelta: 1 })
    expect(boardSyncStatusForTelemetry("rpc-refresh-required")).toBeNull()
  })

  test("释放后忽略迟到回调，手动重试复用原控制器", () => {
    const value = binding()
    value.bound.start()
    value.bound.stop()
    value.context().onRefresh()
    value.context().onState("live")
    expect(value.record).not.toHaveBeenCalled()
    value.bound.retry()
    value.context().onRefresh()
    expect(value.start).toHaveBeenCalledOnce()
    expect(value.stop).toHaveBeenCalledOnce()
    expect(value.retry).toHaveBeenCalledOnce()
    expect(value.record).toHaveBeenCalledOnce()
  })

  test("连接状态映射到原 UI 状态，刷新提示不宣称查询已完成", () => {
    const value = binding()
    value.bound.start()
    for (const state of ["connecting", "live", "retrying", "failed"] as const) value.context().onState(state)
    expect(value.record.mock.calls.map(([entry]) => boardSyncStatusForTelemetry(entry.type)))
      .toEqual(["connecting", "live", "stale", "circuit-open"])
    value.setState("failed")
    expect(value.bound.snapshot()).toEqual({ state: "circuit-open" })
    value.setState("retrying")
    expect(value.bound.snapshot()).toEqual({ state: "recovering" })
  })

  test("真实生成客户端可直接赋给 application source，并通过绑定器报告失败", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => { throw new Error("测试传输断开") })
    const source: BoardRealtimeSource = createHostRealtime({
      apiBaseUrl: "/rpc", webBasePath: "/app/", actor: "test", defaultBoard: "generated",
      serverVersion: "3.1.0", protocolVersion: "v1", webBuildId: "test",
    }, { documentBaseURI: "http://127.0.0.1:8721/app/", maxFailures: 1, fetcher })
    const record = vi.fn<(entry: SyncTelemetryEntry) => void>()
    const bound = bindBoardRealtime(source, { boardId, boardSelector: "generated", record })
    bound.start()
    try {
      await vi.waitFor(() => expect(bound.snapshot()).toEqual({ state: "circuit-open" }))
      expect(fetcher).toHaveBeenCalledOnce()
      expect(record.mock.calls.map(([entry]) => entry.type)).toEqual(["rpc-connecting", "circuit-open"])
      expect(record.mock.calls.every(([entry]) => entry.boardId === boardId && entry.cursor === 0)).toBe(true)
    } finally { bound.stop() }
  })
})
