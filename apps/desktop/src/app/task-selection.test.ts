import { describe, expect, it } from "vitest"

import { reconcileSelectedTaskId, shouldLoadTaskCollection, shouldLoadTaskDetail, shouldOpenTaskDetailSheet } from "./task-selection"

const tasks = [{ id: "t_1" }, { id: "t_2" }]

describe("desktop task selection rules", () => {
  it("keeps null selection instead of auto-selecting the first task", () => {
    expect(reconcileSelectedTaskId(null, tasks)).toBeNull()
  })

  it("keeps a selected task when it still exists in the current task set", () => {
    expect(reconcileSelectedTaskId("t_2", tasks)).toBe("t_2")
  })

  it("clears selection when the selected task disappears from the current task set", () => {
    expect(reconcileSelectedTaskId("t_missing", tasks)).toBeNull()
  })

  it("opens detail only for detail-capable views with a selected task", () => {
    expect(shouldOpenTaskDetailSheet("board", { id: "t_1" })).toBe(true)
    expect(shouldOpenTaskDetailSheet("list", { id: "t_1" })).toBe(true)
    expect(shouldOpenTaskDetailSheet("runs", { id: "t_1" })).toBe(true)
    expect(shouldOpenTaskDetailSheet("map", { id: "t_1" })).toBe(true)
    expect(shouldOpenTaskDetailSheet("runs", null)).toBe(false)
    expect(shouldOpenTaskDetailSheet("events", { id: "t_1" })).toBe(false)
    expect(shouldOpenTaskDetailSheet("maintenance", { id: "t_1" })).toBe(false)
  })

  it("loads the board task collection only for task explorer views", () => {
    expect(shouldLoadTaskCollection("board")).toBe(true)
    expect(shouldLoadTaskCollection("list")).toBe(true)
    expect(shouldLoadTaskCollection("map")).toBe(false)
    expect(shouldLoadTaskCollection("events")).toBe(false)
    expect(shouldLoadTaskCollection("runs")).toBe(false)
  })

  it("loads detail data only when the current view can show the task workbench", () => {
    expect(shouldLoadTaskDetail("board", "t_1")).toBe(true)
    expect(shouldLoadTaskDetail("map", "t_1")).toBe(true)
    expect(shouldLoadTaskDetail("runs", "t_1")).toBe(true)
    expect(shouldLoadTaskDetail("map", null)).toBe(false)
    expect(shouldLoadTaskDetail("events", "t_1")).toBe(false)
  })
})
