import { QueryClient } from "@tanstack/react-query"
import { describe, expect, it, vi } from "vitest"

import { invalidateTaskDetailAndBoard } from "./detail-invalidation"

describe("detail invalidation", () => {
  it("invalidates board, detail, step, and graph queries", async () => {
    const queryClient = new QueryClient()
    const invalidate = vi.spyOn(queryClient, "invalidateQueries")

    await invalidateTaskDetailAndBoard(queryClient, "default", "t_1")

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["tasks", "default"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["stats", "default"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["board-task-map", "default"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-detail", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-steps", "t_1"] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["task-neighborhood", "t_1"] })
  })
})
