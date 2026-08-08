import { describe, expect, test } from "vitest"

import { ExplorerReadError } from "../../lib/api/explorer-read-model"
import { shouldClearMapTaskFromInspector } from "./ExplorerPage.logic"

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
})
