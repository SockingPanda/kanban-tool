import { QueryClient } from "@tanstack/react-query"
import { describe, expect, it, vi } from "vitest"

import { invalidateTaskMutationScope } from "./task-mutation-invalidation"

describe("task mutation invalidation", () => {
  it("keeps comment mutations scoped to the selected task timeline", async () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, "invalidateQueries")

    await invalidateTaskMutationScope({
      board: "default",
      queryClient,
      scope: "timeline",
      selectedTaskId: "t_1",
      taskId: "t_1",
    })

    expect(invalidate.mock.calls.map(([arg]) => arg)).toEqual([
      { queryKey: ["task-events", "t_1"] },
      { queryKey: ["task-comments", "t_1"] },
    ])
  })

  it("refreshes dependency mutations without invalidating unrelated detail buckets", async () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, "invalidateQueries")

    await invalidateTaskMutationScope({
      board: "default",
      queryClient,
      scope: "dependencies",
      selectedTaskId: "t_1",
      taskId: "t_2",
    })

    expect(invalidate.mock.calls.map(([arg]) => arg)).toEqual([
      { queryKey: ["task-dependencies", "t_2"] },
      { queryKey: ["task-neighborhood", "t_2"] },
      { queryKey: ["task-events", "t_2"] },
      { queryKey: ["tasks", "default"] },
      { queryKey: ["board-task-map", "default"] },
    ])
    expect(invalidate).not.toHaveBeenCalledWith({ queryKey: ["task-comments", "t_2"] })
    expect(invalidate).not.toHaveBeenCalledWith({ queryKey: ["task-runs", "t_2"] })
  })

  it("supports board-only invalidation for board switch side effects", async () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, "invalidateQueries")

    await invalidateTaskMutationScope({
      board: "default",
      queryClient,
      scope: "board",
      selectedTaskId: "t_1",
      taskId: null,
    })

    expect(invalidate.mock.calls.map(([arg]) => arg)).toEqual([
      { queryKey: ["tasks", "default"] },
      { queryKey: ["stats", "default"] },
      { queryKey: ["board-task-map", "default"] },
    ])
  })
})
