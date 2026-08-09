import { describe, expect, test } from "vitest"

import { ExplorerReadError } from "../../lib/api/explorer-read-model"
import { asyncReadToken, shouldClearMapTaskFromInspector, visibleAsyncReadState } from "./ExplorerPage.logic"

describe("ExplorerPage task URL authority", () => {
  test("clears only a typed task-not-found for the current map selection", () => {
    const notFound = new ExplorerReadError("http", "请求的任务不存在。", { reason: "task-not-found", status: 404 })
    const crossBoard = new ExplorerReadError("anomaly", "任务不属于当前 board。", { reason: "task-not-found" })
    const anomaly = new ExplorerReadError("anomaly", "任务越过当前 board scope。")

    expect(shouldClearMapTaskFromInspector("map", "t_missing", notFound)).toBe(true)
    expect(shouldClearMapTaskFromInspector("map", "t_other", crossBoard)).toBe(true)
    expect(shouldClearMapTaskFromInspector("map", "t_valid", anomaly)).toBe(false)
    expect(shouldClearMapTaskFromInspector("list", "t_missing", notFound)).toBe(false)
    expect(shouldClearMapTaskFromInspector("map", null, notFound)).toBe(false)
  })

  test("does not expose deferred A data or errors under a B request key", () => {
    const tokenA = asyncReadToken(true, "board-a|t_a", 0)
    const tokenB = asyncReadToken(true, "board-b|t_b", 0)
    const readyA = { data: "task-a", error: null, loading: false, ...tokenA }
    const failedA = { data: null, error: new ExplorerReadError("http", "task missing", { reason: "task-not-found", status: 404 }), loading: false, ...tokenA }

    expect(visibleAsyncReadState(readyA, tokenB, true)).toEqual({ data: null, error: null, loading: true })
    const firstBFrame = visibleAsyncReadState(failedA, tokenB, true)
    expect(firstBFrame).toEqual({ data: null, error: null, loading: true })
    expect(shouldClearMapTaskFromInspector("map", "t_b", firstBFrame.error)).toBe(false)
  })

  test("retains same-key data during a generation retry but hides its old error", () => {
    const tokenA = asyncReadToken(true, "board-a|t_a", 0)
    const retryToken = asyncReadToken(true, "board-a|t_a", 1)
    const readyA = { data: "task-a", error: null, loading: false, ...tokenA }
    const failedA = { data: "task-a", error: new Error("temporary"), loading: false, ...tokenA }

    expect(visibleAsyncReadState(readyA, retryToken, true)).toEqual({ data: "task-a", error: null, loading: true })
    expect(visibleAsyncReadState(failedA, retryToken, true)).toEqual({ data: "task-a", error: null, loading: true })
  })
})
