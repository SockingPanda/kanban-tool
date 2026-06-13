import { describe, expect, it } from "vitest"

import type { Dependencies, Task } from "@/lib/api"

import { dependencyBlockedTodoClass, priorityBadgeLabel, sortBoardColumnTasks, selectedDependencyCountForTask } from "./board-card-state"

describe("board card state", () => {
  it("hides dependency metadata for unselected tasks", () => {
    expect(
      selectedDependencyCountForTask("t_2", {
        selectedTaskId: "t_1",
        detailTaskId: "t_1",
        dependencies: dependencies(1, 1),
        loading: false,
      }),
    ).toBeUndefined()
  })

  it("hides selected dependency metadata while the selected detail is loading", () => {
    expect(
      selectedDependencyCountForTask("t_1", {
        selectedTaskId: "t_1",
        detailTaskId: null,
        dependencies: null,
        loading: true,
      }),
    ).toBeUndefined()
  })

  it("hides selected dependency metadata when detail belongs to a previous selection", () => {
    expect(
      selectedDependencyCountForTask("t_2", {
        selectedTaskId: "t_2",
        detailTaskId: "t_1",
        dependencies: dependencies(0, 3),
        loading: false,
      }),
    ).toBeUndefined()
  })

  it("shows zero dependencies only after current selected dependencies are confirmed", () => {
    expect(
      selectedDependencyCountForTask("t_1", {
        selectedTaskId: "t_1",
        detailTaskId: "t_1",
        dependencies: dependencies(0, 0),
        loading: false,
      }),
    ).toBe(0)
  })

  it("sums current selected parent and child dependencies", () => {
    expect(
      selectedDependencyCountForTask("t_1", {
        selectedTaskId: "t_1",
        detailTaskId: "t_1",
        dependencies: dependencies(2, 3),
        loading: false,
      }),
    ).toBe(5)
  })

  it("labels priority levels and falls back to P3 for legacy values", () => {
    expect(priorityBadgeLabel(7)).toBe("P3")
    expect(priorityBadgeLabel(0)).toBe("P0")
    expect(priorityBadgeLabel(-2)).toBe("P3")
  })

  it("sorts unblocked todo cards before dependency-blocked todo cards", () => {
    const blocked = task("blocked", { status: "todo", dependency_blocked: true, position: 1, created_at: 1 })
    const unblocked = task("unblocked", { status: "todo", dependency_blocked: false, position: 2, created_at: 2 })

    expect(sortBoardColumnTasks([blocked, unblocked], "todo").map((item) => item.id)).toEqual(["unblocked", "blocked"])
  })

  it("does not reorder non-todo columns by dependency-blocked state", () => {
    const blocked = task("blocked", { status: "ready", dependency_blocked: true, position: 1 })
    const unblocked = task("unblocked", { status: "ready", dependency_blocked: false, position: 2 })

    expect(sortBoardColumnTasks([blocked, unblocked], "ready").map((item) => item.id)).toEqual(["blocked", "unblocked"])
  })

  it("adds red border styling only for dependency-blocked todo cards", () => {
    expect(dependencyBlockedTodoClass(task("blocked", { status: "todo", dependency_blocked: true }))).toContain("border-red")
    expect(dependencyBlockedTodoClass(task("ready", { status: "ready", dependency_blocked: true }))).toBeNull()
    expect(dependencyBlockedTodoClass(task("todo", { status: "todo", dependency_blocked: false }))).toBeNull()
  })
})

function dependencies(parentCount: number, childCount: number): Dependencies {
  return {
    parents: Array.from({ length: parentCount }, (_, index) => task(`p_${index}`)),
    children: Array.from({ length: childCount }, (_, index) => task(`c_${index}`)),
  }
}

function task(id: string, overrides: Partial<Task> = {}): Task {
  return { ...baseTask(id), ...overrides }
}

function baseTask(id: string): Task {
  return {
    id,
    board_id: "b_1",
    board_slug: "default",
    ref: `default#${id}`,
    seq: 1,
    title: id,
    description: null,
    status: "ready" as const,
    status_reason: null,
    assignee: null,
    priority: 0,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "test",
    created_at: 1,
    updated_at: 1,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result_json: null,
    metadata_json: "{}",
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
  }
}
