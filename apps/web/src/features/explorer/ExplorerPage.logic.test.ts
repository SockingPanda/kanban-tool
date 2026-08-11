import { describe, expect, test } from "vitest"

import { ExplorerReadError } from "../../lib/api/explorer-read-model"
import { asyncReadToken, inspectorRelationsView, parseTaskDisplay, serializeTaskDisplay, shouldClearMapTaskFromInspector, visibleAsyncReadState, withTaskDisplay } from "./ExplorerPage.logic"

describe("Tasks list display URL contract", () => {
  test("uses list as the canonical default and recognizes only table", () => {
    expect(parseTaskDisplay("task=t_1&display=table&unknown=keep")).toBe("table")
    expect(parseTaskDisplay("display=grouped")).toBe("list")
    expect(parseTaskDisplay("")).toBe("list")
    expect(serializeTaskDisplay("list")).toBe("")
    expect(serializeTaskDisplay("table")).toBe("display=table")
  })

  test("clones display changes without dropping task, map controls, or unknown query", () => {
    const table = withTaskDisplay("task=t_1&filter=blocked&zoom=1.2&unknown=keep", "table")
    expect(table.toString()).toBe("task=t_1&filter=blocked&zoom=1.2&unknown=keep&display=table")
    const list = withTaskDisplay(table, "list")
    expect(list.toString()).toBe("task=t_1&filter=blocked&zoom=1.2&unknown=keep")
  })
})

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

  test("dedupes relation rows at the adapter boundary while preserving canonical order", () => {
    const task = (id: string, title = id) => ({ id, ref: `default#${id}`, title, status: "todo" as const })
    const view = inspectorRelationsView({
      comments: [
        { id: "c_1", author: "alice", kind: "note", body: "first", created_at: 1, metadata: {} },
        { id: "c_1", author: "alice", kind: "note", body: "duplicate", created_at: 2, metadata: {} },
        { id: "c_2", author: "bob", kind: "signal", body: "second", created_at: 3, metadata: {} },
      ],
      dependencies: {
        parents: [task("t_parent", "first parent"), task("t_parent", "duplicate parent")],
        children: [task("t_child", "first child"), task("t_child", "duplicate child")],
      },
      steps: {
        steps: [
          { id: "s_1", title: "first step", body: null, required: true, status: "todo", linked_task: null },
          { id: "s_1", title: "duplicate step", body: null, required: true, status: "todo", linked_task: null },
          { id: "s_2", title: "second step", body: null, required: false, status: "done", linked_task: null },
        ],
        execution_plan: { state: "planned", reason: null },
      },
    })

    expect(view.comments.map((comment) => comment.id)).toEqual(["c_1", "c_2"])
    expect(view.comments[0]?.body).toBe("first")
    expect(view.dependencies.parents.map((parent) => parent.id)).toEqual(["t_parent"])
    expect(view.dependencies.children.map((child) => child.id)).toEqual(["t_child"])
    expect(view.steps.steps.map((step) => step.id)).toEqual(["s_1", "s_2"])
    expect(view.steps.steps[0]?.title).toBe("first step")
  })
})
