import { describe, expect, test } from "vitest"

import {
  INSPECTOR_MUTATION_KINDS,
  createCommittedMutationEvent,
  createTaskClaimTokenStore,
  inspectorMutationKey,
  inspectorMutationScopeKey,
  type InspectorTaskMutationClient,
  type InspectorTaskLabelSuggestionClient,
  type TaskInspectorMutationCommitted,
  type TaskInspectorMutationScope,
  type TaskInspectorMutationSurface,
} from "./task-inspector-mutation-state"

describe("Task Inspector mutation surface contract", () => {
  test("keeps the committed event taxonomy exact and task-scoped", () => {
    expect(INSPECTOR_MUTATION_KINDS).toEqual([
      "edit",
      "transition",
      "dependency",
      "step",
      "comment",
      "label",
      "attachment",
    ])
    expect(createCommittedMutationEvent("edit", "t_1")).toEqual({ kind: "edit", taskId: "t_1" })
    expect(createCommittedMutationEvent("attachment", "t_1")).toEqual({ kind: "attachment", taskId: "t_1" })
  })

  test("derives stable scoped pending keys from an operation and task", () => {
    expect(inspectorMutationKey("saveTask", "t_1")).toBe("saveTask:t_1")
    expect(inspectorMutationKey("dependency", "t_1")).toBe("dependency:t_1")
    expect(inspectorMutationScopeKey({ identity: "runtime\u0000b_default\u0000t_1", boardId: "b_default", taskId: "t_1" }))
      .toBe("runtime\u0000b_default\u0000t_1")
  })

  test("allows Board and Inspector surfaces to share one claim token store", () => {
    const store = createTaskClaimTokenStore()
    store.set("t_1", "claim-token")
    expect(store.get("t_1")).toBe("claim-token")

    const inspectorSurface = { claimTokens: store } as Pick<TaskInspectorMutationSurface, "claimTokens">
    expect(inspectorSurface.claimTokens?.get("t_1")).toBe("claim-token")

    store.delete("t_1")
    expect(store.get("t_1")).toBeNull()
  })

  test("keeps the mutation client typed to the canonical operation subset", () => {
    const client: InspectorTaskMutationClient = {
      updateTask: async () => ({ data: {} as never }),
      transitionTask: async () => ({ data: {} as never }),
      addDependency: async () => ({ data: {} as never }),
      removeDependency: async () => ({ data: {} as never }),
      createStep: async () => ({ data: {} as never }),
      markExecutionPlanNotRequired: async () => ({ data: {} as never }),
      addTaskLabel: async () => ({ data: {} as never }),
      removeTaskLabel: async () => ({ data: {} as never }),
      createComment: async () => ({ data: {} as never }),
      createAttachment: async () => ({ data: {} as never }),
      deleteAttachment: async () => ({ data: {} as never }),
    }

    expect(Object.keys(client)).toEqual([
      "updateTask",
      "transitionTask",
      "addDependency",
      "removeDependency",
      "createStep",
      "markExecutionPlanNotRequired",
      "addTaskLabel",
      "removeTaskLabel",
      "createComment",
      "createAttachment",
      "deleteAttachment",
    ])
  })

  test("keeps scoped canonical reload and optional suggestion seams typed", () => {
    const event: TaskInspectorMutationCommitted = { kind: "comment", taskId: "t_1" }
    const scope: TaskInspectorMutationScope = { identity: "runtime\u0000b_default\u0000t_1", boardId: "b_default", taskId: "t_1" }
    const reload = async (committed: TaskInspectorMutationCommitted, current: TaskInspectorMutationScope) => {
      expect(committed).toEqual(event)
      expect(current).toEqual(scope)
    }
    const suggestions: InspectorTaskLabelSuggestionClient = {
      suggestTaskLabels: async (_taskId, query, options) => {
        expect(query).toEqual({ limit: 5 })
        expect(options?.signal).toBeUndefined()
        return { data: {} as never }
      },
    }
    const surface: Pick<TaskInspectorMutationSurface, "onCanonicalReload" | "suggestTaskLabels"> = {
      onCanonicalReload: reload,
      suggestTaskLabels: suggestions.suggestTaskLabels,
    }
    expect(surface.onCanonicalReload).toBe(reload)
    expect(surface.suggestTaskLabels).toBe(suggestions.suggestTaskLabels)
  })
})
