import { QueryClient } from "@tanstack/react-query"
import { describe, expect, it, vi } from "vitest"

import { invalidateTaskDetailAndBoard, invalidateTaskTimelineQueries } from "./detail-invalidation"

describe("detail invalidation", () => {
  it("invalidates board, detail, step, and graph queries", async () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, "invalidateQueries")

    await invalidateTaskDetailAndBoard(queryClient, "default", "t_1")

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["tasks", "default"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["stats", "default"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["board-task-map", "default"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-detail", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-dependencies", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-steps", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-neighborhood", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-runs", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-events", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-comments", "t_1"] })
  })

  it("invalidates only task timeline child queries for comment refreshes", async () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, "invalidateQueries")

    await invalidateTaskTimelineQueries(queryClient, "t_1")

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-events", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-comments", "t_1"] })
    expect(invalidate).not.toHaveBeenCalledWith({ queryKey: ["task-steps", "t_1"] })
    expect(invalidate).not.toHaveBeenCalledWith({ queryKey: ["task-neighborhood", "t_1"] })
  })

  it("starts timeline invalidations in parallel", async () => {
    const queryClient = new QueryClient()
    const pending: Array<() => void> = []
    const invalidate = vi.spyOn(queryClient, "invalidateQueries").mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          pending.push(resolve)
        }),
    )

    const invalidation = invalidateTaskTimelineQueries(queryClient, "t_1")

    expect(invalidate).toHaveBeenCalledTimes(2)
    pending.forEach((resolve) => resolve())
    await invalidation
  })
})
