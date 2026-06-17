import { describe, expect, it } from "vitest"

import type { CommentRecord } from "@/lib/api"

import { COMMENT_PAGE_SIZE, commentPageState, sortedComments } from "./comment-list-state"

describe("comment list state", () => {
  it("sorts newest comments first by default contract", () => {
    expect(sortedComments([comment(1), comment(3), comment(2)], "newest").map((item) => item.id)).toEqual([
      "c_3",
      "c_2",
      "c_1",
    ])
  })

  it("shows ten comments per page and exposes the next page", () => {
    const state = commentPageState({ comments: comments(12), page: 0, sortOrder: "newest" })

    expect(state.comments).toHaveLength(COMMENT_PAGE_SIZE)
    expect(state.comments.map((item) => item.id)).toEqual([
      "c_12",
      "c_11",
      "c_10",
      "c_9",
      "c_8",
      "c_7",
      "c_6",
      "c_5",
      "c_4",
      "c_3",
    ])
    expect(state.hasPreviousPage).toBe(false)
    expect(state.hasNextPage).toBe(true)
    expect(state.pageCount).toBe(2)
  })

  it("keeps all comments reachable on later pages", () => {
    const state = commentPageState({ comments: comments(12), page: 1, sortOrder: "newest" })

    expect(state.comments.map((item) => item.id)).toEqual(["c_2", "c_1"])
    expect(state.hasPreviousPage).toBe(true)
    expect(state.hasNextPage).toBe(false)
  })

  it("does not create pagination for ten or fewer comments", () => {
    const state = commentPageState({ comments: comments(10), page: 0, sortOrder: "newest" })

    expect(state.pageCount).toBe(1)
    expect(state.hasPreviousPage).toBe(false)
    expect(state.hasNextPage).toBe(false)
    expect(state.comments).toHaveLength(10)
  })

  it("switches to oldest-first ordering without losing comments", () => {
    const state = commentPageState({ comments: comments(12), page: 0, sortOrder: "oldest" })

    expect(state.comments.map((item) => item.id)).toEqual([
      "c_1",
      "c_2",
      "c_3",
      "c_4",
      "c_5",
      "c_6",
      "c_7",
      "c_8",
      "c_9",
      "c_10",
    ])
    expect(state.total).toBe(12)
  })

  it("clamps pages past the multi-page boundary", () => {
    const state = commentPageState({ comments: comments(12), page: 99, sortOrder: "oldest" })

    expect(state.page).toBe(1)
    expect(state.comments.map((item) => item.id)).toEqual(["c_11", "c_12"])
  })

  it("returns an empty page state for an empty comment list", () => {
    const state = commentPageState({ comments: [], page: 0, sortOrder: "newest" })

    expect(state).toMatchObject({
      comments: [],
      page: 0,
      pageCount: 1,
      hasPreviousPage: false,
      hasNextPage: false,
      total: 0,
    })
  })
})

function comments(count: number) {
  return Array.from({ length: count }, (_, index) => comment(index + 1))
}

function comment(index: number): CommentRecord {
  return {
    id: `c_${index}`,
    board_id: "b_1",
    task_id: "t_1",
    author: "codex",
    author_type: "agent",
    agent_type: "codex",
    body: `Comment ${index}`,
    kind: "note",
    metadata_json: "{}",
    created_at: index,
  }
}
