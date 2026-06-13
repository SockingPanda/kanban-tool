import { describe, expect, it } from "vitest"

import { createBoardSwitchReset } from "./board-switch-state"

describe("createBoardSwitchReset", () => {
  it("clears board-scoped task state and returns a fresh config", () => {
    const reset = createBoardSwitchReset({
      config: {
        apiBaseUrl: "http://127.0.0.1:8721",
        dbPath: "test.db",
        actor: "desktop-test",
        board: "default",
      },
      selectedId: "t_selected",
      pageOffset: 200,
      newTitle: "draft title",
      newDescription: "draft description",
      blockReason: "blocked",
      dependencyInput: "t_parent",
      commentBody: "comment",
      draftState: {
        taskId: "t_selected",
        dirty: true,
        draft: {
          title: "edited",
          description: "edited description",
          assignee: "me",
          priority: "0",
          dueAt: "2026-06-13T08:00",
          scheduledAt: "",
        },
      },
      claimTokens: { t_selected: "token" },
      lastRefreshAt: 42,
      error: "old error",
    })

    expect(reset).toEqual({
      config: {
        apiBaseUrl: "http://127.0.0.1:8721",
        dbPath: "test.db",
        actor: "desktop-test",
        board: "default",
      },
      selectedId: null,
      pageOffset: 0,
      newTitle: "",
      newDescription: "",
      blockReason: "",
      dependencyInput: "",
      commentBody: "",
      draftState: null,
      claimTokens: {},
      lastRefreshAt: null,
      error: null,
    })
  })
})
