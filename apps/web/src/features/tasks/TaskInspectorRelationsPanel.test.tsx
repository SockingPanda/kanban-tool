import { describe, expect, test, vi } from 'vitest';
import { __test } from './TaskInspectorRelationsPanel.logic';
describe("TaskInspectorRelationsPanel input seams", () => {
  test("builds exact typed comment input", () => {
    expect(__test.commentInput(" decision ", " Keep this ")).toEqual({
      kind: "decision",
      body: "Keep this",
    })
  })

  test("builds exact typed step and plan inputs", () => {
    expect(__test.stepInput(" Verify ", " Body ", true)).toEqual({ title: "Verify", body: "Body", required: true })
    expect(__test.stepInput(" Link ", "", false, " default#2 ")).toEqual({ title: "Link", required: false, linked_task_ref: "default#2" })
    expect(__test.stepSubmission(" Create ", " Body ", true, "default#2", "create")).toEqual({ operation: "createStep", input: { title: "Create", body: "Body", required: true } })
    expect(__test.stepSubmission(" Link ", "", false, "t_2", "link")).toEqual({ operation: "linkStep", input: { title: "Link", required: false, linked_task_ref: "t_2" } })
    expect(__test.stepSubmission(" ", "Body", true, "t_2", "create")).toBeNull()
    expect(__test.stepSubmission("Link", "Body", true, "", "link")).toBeNull()
    expect(__test.planInput(" manual execution ")).toEqual({ reason: "manual execution" })
  })

  test("sorts and pages comments through the same seam used by the controls", () => {
    const comments = [
      { id: "c_old", createdAt: 10 },
      { id: "c_new", createdAt: 20 },
      { id: "c_mid", createdAt: 15 },
    ]
    expect(__test.commentPageState(comments, 0, 2, "newest").comments.map((comment) => comment.id)).toEqual(["c_new", "c_mid"])
    expect(__test.commentPageState(comments, 1, 2, "newest").comments.map((comment) => comment.id)).toEqual(["c_old"])
    expect(__test.commentPageState(comments, 0, 2, "oldest").comments.map((comment) => comment.id)).toEqual(["c_old", "c_mid"])
  })

  test("formats comment timestamps as localized labels with valid machine values", () => {
    const rendered = __test.formatCommentDateTime(0, "zh")
    expect(rendered.iso).toBe("1970-01-01T00:00:00.000Z")
    expect(rendered.label).not.toBe("0")
    expect(__test.formatCommentDateTime(Number.NaN, "en")).toEqual({ label: "—", iso: "" })
  })

  test("clears drafts only after commit, including committed-but-unreconciled writes", () => {
    expect(__test.shouldClearDraft({ committed: false, reconciled: false })).toBe(false)
    expect(__test.shouldClearDraft({ committed: false, reconciled: true })).toBe(false)
    expect(__test.shouldClearDraft({ committed: true, reconciled: false })).toBe(true)
    expect(__test.shouldClearDraft({ committed: true, reconciled: true })).toBe(true)
    expect(__test.shouldClearDraft({ committed: true, reconciled: true }, false)).toBe(false)
    expect(__test.shouldClearRetryDraft({ committed: false, reconciled: false }, true)).toBe(false)
    expect(__test.shouldClearRetryDraft({ committed: true, reconciled: false }, true)).toBe(true)
    expect(__test.shouldClearRetryDraft({ committed: true, reconciled: false }, false)).toBe(false)
    expect(__test.shouldClearRetryDraft({ committed: true, reconciled: true }, false)).toBe(false)
  })

  test("matches retry intents only while the current draft and scope epoch remain equal", () => {
    const resolver = (selector: string) => selector === "default#2" ? "t_2" : null
    expect(__test.commentDraftMatchesRetry("note", "retry me", { body: "retry me", kind: "note" })).toBe(true)
    expect(__test.commentDraftMatchesRetry("note", "edited", { body: "retry me", kind: "note" })).toBe(false)
    expect(__test.dependencyDraftMatchesRetry("default#2", "t_2", resolver)).toBe(true)
    expect(__test.dependencyDraftMatchesRetry("default#3", "t_2", resolver)).toBe(false)
    expect(__test.stepDraftMatchesRetry("Link", "Body", true, "default#2", { title: "Link", body: "Body", required: true, linked_task_ref: "t_2" }, resolver)).toBe(true)
    expect(__test.stepDraftMatchesRetry("Changed", "Body", true, "default#2", { title: "Link", body: "Body", required: true, linked_task_ref: "t_2" }, resolver)).toBe(false)
    expect(__test.planDraftMatchesRetry("why", "why")).toBe(true)
    expect(__test.planDraftMatchesRetry("changed", "why")).toBe(false)
    expect(__test.scopeEpochMatches({ identity: "runtime\u0000t1", generation: 1, taskId: "t1" }, { identity: "runtime\u0000t1", generation: 2, taskId: "t1" })).toBe(false)
    expect(__test.scopeEpochMatches({ identity: "runtime\u0000t1", generation: 1, taskId: "t1" }, { identity: "runtime\u0000t1", generation: 1, taskId: "t2" })).toBe(false)
    expect(__test.scopeEpochMatches({ identity: "runtime\u0000t1", generation: 1, taskId: "t1" }, { identity: "runtime\u0000t1", generation: 1, taskId: "t1" })).toBe(true)
  })

  test("resolves only active-board ids/refs before a mutation, with no call for unresolved input", () => {
    const resolver = vi.fn((selector: string) => selector === "t_known" ? "t_known" : selector === "default#2" ? "t_2" : null)
    expect(__test.resolveTaskSelector(" t_unknown ", resolver)).toBeNull()
    expect(resolver).toHaveBeenCalledWith("t_unknown")
    expect(__test.resolveTaskSelector(" t_known ", resolver)).toBe("t_known")
    expect(__test.resolveTaskSelector(" default#2 ", resolver)).toBe("t_2")
    const addDependency = vi.fn()
    const unresolved = __test.resolveTaskSelector("t_cross_board", resolver)
    if (unresolved) addDependency(unresolved)
    expect(addDependency).not.toHaveBeenCalled()
  })
})
