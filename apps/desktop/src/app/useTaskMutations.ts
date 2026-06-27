import { useCallback, useMemo, useState } from "react"
import { useMutation, type QueryClient } from "@tanstack/react-query"

import { invalidateTaskDetailAndBoard } from "@/features/task-detail/detail-invalidation"
import {
  parseDateInput,
  reconcileSavedTaskDraft,
  reconcileTaskDraft,
  type TaskDraftState,
  type TaskEditDraft,
} from "@/features/task-detail/task-draft"
import type { ClaimResponse, KanbanApi, RuntimeConfig, Task } from "@/lib/api"
import { reconcileClaimTokenForTask } from "@/lib/claim-tokens"
import { queryKeys } from "@/lib/query-keys"

export type RunActionOptions = {
  label?: string
  fallbackTaskId?: string | null
}

export function useTaskMutations({
  api,
  commentBody,
  config,
  creation,
  dependencyInput,
  draftState,
  queryClient,
  selectedId,
  selectedTask,
  setClaimTokens,
  setCommentBody,
  setDependencyInput,
  setDraftState,
  setError,
  setLabelSuggestionsRequested,
  setSelectedId,
}: {
  api: KanbanApi | null
  commentBody: string
  config: RuntimeConfig | null
  creation: {
    title: string
    description: string
    firstStepTitle: string
    reset: () => void
  }
  dependencyInput: string
  draftState: TaskDraftState | null
  queryClient: QueryClient
  selectedId: string | null
  selectedTask: Task | null
  setClaimTokens: React.Dispatch<React.SetStateAction<Record<string, string>>>
  setCommentBody: (value: string) => void
  setDependencyInput: (value: string) => void
  setDraftState: React.Dispatch<React.SetStateAction<TaskDraftState | null>>
  setError: (value: string | null) => void
  setLabelSuggestionsRequested: (value: boolean) => void
  setSelectedId: (value: string | null) => void
}) {
  const [pendingAction, setPendingAction] = useState<string | null>(null)
  const actionMutation = useMutation({
    mutationFn: (action: () => Promise<unknown>) => action(),
  })

  const invalidateTaskData = useCallback(
    async (taskId: string | null) => {
      if (!api) return
      await invalidateTaskDetailAndBoard(queryClient, api.board, taskId)
      if (taskId) {
        queryClient.removeQueries({ queryKey: queryKeys.taskLabelSuggestions(taskId) })
        if (taskId === selectedId) setLabelSuggestionsRequested(false)
      }
    },
    [api, queryClient, selectedId, setLabelSuggestionsRequested],
  )

  const runAction = useCallback(
    async (action: () => Promise<unknown>, options: RunActionOptions | string = "action") => {
      const label = typeof options === "string" ? options : options.label ?? "action"
      const fallbackTaskId = typeof options === "string" ? selectedId : options.fallbackTaskId
      setPendingAction(label)
      setError(null)
      try {
        const result = await actionMutation.mutateAsync(action)
        if (isClaimResponse(result)) {
          setClaimTokens((current) => ({ ...current, [result.task.id]: result.claim_token }))
          await invalidateTaskData(result.task.id)
          return result
        }
        if (isTask(result)) {
          setClaimTokens((current) => reconcileClaimTokenForTask(current, result, config?.actor ?? null))
          await invalidateTaskData(result.id)
          return result
        }
        await invalidateTaskData(fallbackTaskId ?? null)
        return result
      } catch (err) {
        setError(errorMessage(err))
      } finally {
        setPendingAction(null)
      }
    },
    [actionMutation, config?.actor, invalidateTaskData, selectedId, setClaimTokens, setError],
  )

  const createTask = useCallback(async () => {
    if (!api || !creation.title.trim()) return false
    const result = await runAction(async () => {
      const task = await api.createTask({
        title: creation.title.trim(),
        description: creation.description.trim() || undefined,
      })
      if (creation.firstStepTitle.trim()) {
        await api.createStep(task.id, { title: creation.firstStepTitle.trim(), required: true })
      }
      setSelectedId(task.id)
      creation.reset()
      return task
    }, "create")
    return isTask(result)
  }, [api, creation, runAction, setSelectedId])

  const addDependency = useCallback(async () => {
    if (!api || !selectedTask || !dependencyInput.trim()) return
    const taskId = selectedTask.id
    await runAction(async () => {
      const result = await api.addDependency(taskId, dependencyInput.trim())
      setDependencyInput("")
      return result
    }, { label: "dependency", fallbackTaskId: taskId })
  }, [api, dependencyInput, runAction, selectedTask, setDependencyInput])

  const removeDependency = useCallback(
    async (parentTaskId: string) => {
      if (!api || !selectedTask) return
      const taskId = selectedTask.id
      await runAction(async () => api.removeDependency(taskId, parentTaskId), { label: "dependency", fallbackTaskId: taskId })
    },
    [api, runAction, selectedTask],
  )

  const saveTask = useCallback(async () => {
    if (!api || !selectedTask || !draftState) return false
    if (draftState.taskId !== selectedTask.id) return false
    if (!draftState.draft.title.trim()) return false
    const taskId = selectedTask.id
    const draft = draftState.draft
    const result = await runAction(async () => {
      const updated = await api.updateTask(taskId, {
        title: draft.title.trim(),
        description: draft.description.trim() || null,
        assignee: draft.assignee.trim() || null,
        priority: Number(draft.priority),
        due_at: parseDateInput(draft.dueAt),
        scheduled_at: parseDateInput(draft.scheduledAt),
      })
      setDraftState((current) => reconcileSavedTaskDraft(current, updated))
      return updated
    }, { label: "save", fallbackTaskId: taskId })
    return isTask(result)
  }, [api, draftState, runAction, selectedTask, setDraftState])

  const cancelTaskEdit = useCallback(() => {
    setDraftState((current) => reconcileTaskDraft(current, selectedTask, { force: true }))
  }, [selectedTask, setDraftState])

  const addComment = useCallback(async () => {
    if (!api || !selectedTask || !commentBody.trim()) return
    const taskId = selectedTask.id
    await runAction(async () => {
      const result = await api.createComment(taskId, commentBody.trim())
      setCommentBody("")
      return result
    }, { label: "comment", fallbackTaskId: taskId })
  }, [api, commentBody, runAction, selectedTask, setCommentBody])

  const updateDraft = useCallback(
    (draft: TaskEditDraft) => {
      setDraftState((current) => {
        if (current) return { ...current, draft, dirty: true }
        if (!selectedTask) return null
        return { taskId: selectedTask.id, draft, dirty: true }
      })
    },
    [selectedTask, setDraftState],
  )

  return useMemo(
    () => ({
      addComment,
      addDependency,
      cancelTaskEdit,
      createTask,
      pendingAction,
      removeDependency,
      runAction,
      saveTask,
      updateDraft,
    }),
    [
      addComment,
      addDependency,
      cancelTaskEdit,
      createTask,
      pendingAction,
      removeDependency,
      runAction,
      saveTask,
      updateDraft,
    ],
  )
}

function isClaimResponse(value: unknown): value is ClaimResponse {
  return Boolean(value && typeof value === "object" && "claim_token" in value)
}

function isTask(value: unknown): value is Task {
  return Boolean(value && typeof value === "object" && "id" in value && "status" in value)
}

function errorMessage(err: unknown) {
  if (err instanceof Error) return err.message
  return String(err)
}
