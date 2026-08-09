import { useEffect, useRef, useState, type DragEvent, type KeyboardEvent } from "react"

import type {
  BoardColumnViewModel,
  BoardMessages,
  BoardTaskViewModel,
  BoardViewModel,
} from "./types"
import {
  isMutationConflict,
  executeBoardTaskTransition,
  moveTaskOptimistically,
  rollbackTaskOptimistically,
  updateTaskDescriptionOptimistically,
  transitionCommandForTask,
  transitionForTaskTarget,
  transitionForTarget,
  updateTaskOptimistically,
  type BoardTaskMutationSurface,
  type BoardTaskTransitionOption,
} from "./task-mutation-state"

export type MutationDialog =
  | { readonly kind: "create"; readonly title: string; readonly taskId: string; readonly idempotencyKey: string }
  | { readonly kind: "edit"; readonly taskId: string; readonly title: string }
  | {
      readonly kind: "transition"
      readonly taskId: string
      readonly option: BoardTaskTransitionOption
      readonly reason: string
      readonly description: string
      readonly confirmed: boolean
    }

export type RetryIntent =
  | { readonly kind: "reload" }
  | { readonly kind: "create"; readonly title: string; readonly taskId: string; readonly idempotencyKey: string }
  | { readonly kind: "edit"; readonly taskId: string; readonly title: string }
  | {
      readonly kind: "transition"
      readonly taskId: string
      readonly option: BoardTaskTransitionOption
      readonly reason: string
      readonly description: string
      readonly confirmed: boolean
    }

export interface MutationNotice {
  readonly kind: "error" | "conflict" | "stale"
  readonly message: string
}

export interface BoardTaskMutationController {
  readonly model: BoardViewModel
  readonly dialog: MutationDialog | null
  readonly notice: MutationNotice | null
  readonly retryIntent: RetryIntent | null
  readonly grabbedTaskId: string | null
  readonly dragAnnouncement: string
  readonly isPending: (key: string) => boolean
  readonly openCreate: (trigger?: HTMLElement | null) => void
  readonly openEdit: (task: BoardTaskViewModel, trigger?: HTMLElement | null) => void
  readonly openTransition: (task: BoardTaskViewModel, option: BoardTaskTransitionOption, trigger?: HTMLElement | null) => void
  readonly claimTokenForTask: (taskId: string) => string | null
  readonly setDialogTitle: (title: string) => void
  readonly setDialogReason: (reason: string) => void
  readonly setDialogDescription: (description: string) => void
  readonly setDialogConfirmed: (confirmed: boolean) => void
  readonly submitDialog: () => void
  readonly closeDialog: () => void
  readonly retryMutation: () => void
  readonly onDragStart: (taskId: string, event: DragEvent<HTMLElement>) => void
  readonly onDragEnd: (taskId: string) => void
  readonly onDragOver: (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => void
  readonly onDrop: (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => void
  readonly onTaskKeyDown: (task: BoardTaskViewModel, event: KeyboardEvent<HTMLElement>) => void
  readonly onTaskRef: (taskId: string, element: HTMLElement | null) => void
  readonly clearGrab: () => void
}

export function keyboardTransitionForDirection(
  columns: readonly BoardColumnViewModel[],
  taskStatus: BoardTaskViewModel["status"],
  direction: "previous" | "next",
): BoardTaskTransitionOption | null {
  const visibleColumns = columns
    .filter((column) => !column.hidden)
    .slice()
    .sort((left, right) => left.position - right.position)
  const currentIndex = visibleColumns.findIndex((column) => column.status === taskStatus)
  if (currentIndex < 0) return null
  const targetIndex = currentIndex + (direction === "next" ? 1 : -1)
  const target = visibleColumns[targetIndex]
  return target === undefined ? null : transitionForTarget(taskStatus, target.status)
}

export function updatePendingMutation(
  current: ReadonlySet<string>,
  key: string,
  pending: boolean,
): { readonly accepted: boolean; readonly next: ReadonlySet<string> } {
  if (pending && current.has(key)) return { accepted: false, next: current }
  const next = new Set(current)
  if (pending) next.add(key)
  else next.delete(key)
  return { accepted: true, next }
}

function taskForId(model: BoardViewModel, taskId: string): BoardTaskViewModel | null {
  for (const tasks of Object.values(model.tasksByStatus)) {
    const task = tasks.find((candidate) => candidate.id === taskId)
    if (task !== undefined) return task
  }
  return null
}

export function transitionInput(option: BoardTaskTransitionOption, reason: string): Readonly<Record<string, unknown>> {
  return option.requiresReason ? { reason } : {}
}

function clientUuid(prefix: string): string {
  const uuid = typeof globalThis.crypto?.randomUUID === "function" ? globalThis.crypto.randomUUID() : `${Date.now()}-${Math.random()}`
  return `${prefix}${uuid}`
}

function mutationMessage(error: unknown, copy: BoardMessages, fallback: string): string {
  if (typeof error === "object" && error !== null) {
    const candidate = error as { readonly status?: unknown; readonly apiError?: { readonly code?: unknown } }
    const code = candidate.apiError?.code
    if (code === "claim_token_mismatch" || code === "claim_conflict" || code === "conflict" || code === "idempotency_conflict" || candidate.status === 409) {
      return copy.conflictDescription
    }
    if (candidate.status === 401 || candidate.status === 403) return copy.mutationUnauthorized
    if (candidate.status === 404 || code === "not_found") return copy.mutationNotFound
    if (typeof candidate.status === "number" && candidate.status >= 500) return copy.mutationUnavailable
  }
  // Never put transport/Error.message in the DOM: only stable product copy is exposed.
  return fallback
}

export function useBoardTaskMutationController(
  baseModel: BoardViewModel | null,
  surface: BoardTaskMutationSurface | undefined,
  columns: readonly BoardColumnViewModel[],
  copy: BoardMessages,
): BoardTaskMutationController | null {
  const [optimisticModel, setOptimisticModel] = useState<BoardViewModel | null>(baseModel)
  const [pendingKeys, setPendingKeys] = useState<ReadonlySet<string>>(() => new Set())
  const [dialog, setDialog] = useState<MutationDialog | null>(null)
  const [notice, setNotice] = useState<MutationNotice | null>(null)
  const [retryIntent, setRetryIntent] = useState<RetryIntent | null>(null)
  const [grabbedTaskId, setGrabbedTaskId] = useState<string | null>(null)
  const [dragAnnouncement, setDragAnnouncement] = useState(copy.grabTask)
  const pendingRef = useRef<ReadonlySet<string>>(new Set())
  const taskRefs = useRef(new Map<string, HTMLElement>())
  const claimTokensRef = useRef(new Map<string, string>())
  const dialogTriggerRef = useRef<HTMLElement | null>(null)
  const dragStateRef = useRef<{ readonly taskId: string; readonly token: string } | null>(null)
  const mountedRef = useRef(true)
  const boardIdentityRef = useRef<string | null>(baseModel?.board?.id ?? null)
  const boardEffectIdentityRef = useRef<string | null>(baseModel?.board?.id ?? null)
  const mutationGenerationRef = useRef(0)
  const optimisticDirtyRef = useRef(false)
  const internalDragMime = "application/x-kanban-task"

  // Render-time fence prevents a late A mutation from reaching B before the
  // effect that resets local state gets a chance to run.
  const renderBoardIdentity = baseModel?.board?.id ?? null
  if (boardIdentityRef.current !== renderBoardIdentity) {
    boardIdentityRef.current = renderBoardIdentity
    mutationGenerationRef.current += 1
    pendingRef.current = new Set()
    claimTokensRef.current.clear()
    dragStateRef.current = null
    optimisticDirtyRef.current = false
  }

  useEffect(() => () => {
    mountedRef.current = false
    claimTokensRef.current.clear()
    dragStateRef.current = null
    optimisticDirtyRef.current = false
  }, [])

  useEffect(() => {
    const nextBoardIdentity = baseModel?.board?.id ?? null
    if (boardEffectIdentityRef.current === nextBoardIdentity) return
    boardEffectIdentityRef.current = nextBoardIdentity
    pendingRef.current = new Set()
    claimTokensRef.current.clear()
    dragStateRef.current = null
    optimisticDirtyRef.current = false
    dialogTriggerRef.current = null
    setPendingKeys(new Set())
    setOptimisticModel(baseModel)
    setDialog(null)
    setNotice(null)
    setRetryIntent(null)
    setGrabbedTaskId(null)
    setDragAnnouncement(copy.grabTask)
  }, [baseModel, copy.grabTask])

  // A failed canonical reconcile deliberately keeps the optimistic snapshot visible.
  useEffect(() => {
    if (baseModel !== null && pendingRef.current.size === 0) {
      optimisticDirtyRef.current = false
      setOptimisticModel(baseModel)
    }
  }, [baseModel])

  const optimisticMatchesBoard = optimisticModel?.board.id === renderBoardIdentity
  const activeModel = (optimisticMatchesBoard && (pendingKeys.size > 0 || optimisticDirtyRef.current) ? optimisticModel : baseModel) ?? baseModel

  const setPending = (key: string, pending: boolean) => {
    const update = updatePendingMutation(pendingRef.current, key, pending)
    pendingRef.current = update.next
    setPendingKeys(update.next)
  }

  const reloadCanonical = async () => {
    await surface?.onCanonicalReload?.()
  }

  const isCurrentMutation = (generation: number) => mountedRef.current && mutationGenerationRef.current === generation

  const closeDialogState = () => {
    setDialog(null)
    const trigger = dialogTriggerRef.current
    dialogTriggerRef.current = null
    if (trigger !== null) queueMicrotask(() => trigger.focus())
  }

  const reconcileAfterMutation = async (): Promise<boolean> => {
    try {
      await reloadCanonical()
      return true
    } catch {
      return false
    }
  }

  const runCanonicalReload = async () => {
    if (surface === undefined || pendingRef.current.has("reload")) return
    const generation = mutationGenerationRef.current
    setPending("reload", true)
    try {
      await reloadCanonical()
      if (isCurrentMutation(generation)) {
        setNotice(null)
        setRetryIntent(null)
        optimisticDirtyRef.current = false
      }
    } catch (error) {
      if (isCurrentMutation(generation)) {
        setNotice({ kind: "stale", message: mutationMessage(error, copy, copy.reconcileStale) })
        setRetryIntent({ kind: "reload" })
      }
    } finally {
      if (isCurrentMutation(generation)) setPending("reload", false)
    }
  }

  const runCreate = async (
    attempt: { readonly title: string; readonly taskId: string; readonly idempotencyKey: string },
  ) => {
    if (surface === undefined || attempt.title.trim().length === 0 || pendingRef.current.has("create")) return
    const generation = mutationGenerationRef.current
    setPending("create", true)
    setNotice(null)
    try {
      await surface.client.createTask({ title: attempt.title.trim(), task_id: attempt.taskId, idempotency_key: attempt.idempotencyKey })
    } catch (error) {
      if (isCurrentMutation(generation)) {
        if (isMutationConflict(error)) {
          const reloaded = await reconcileAfterMutation()
          if (!isCurrentMutation(generation)) return
          setRetryIntent(reloaded ? { kind: "create", ...attempt } : { kind: "reload" })
          setNotice({ kind: reloaded ? "conflict" : "stale", message: reloaded ? copy.conflictDescription : copy.reconcileStale })
        } else {
          setNotice({ kind: "error", message: mutationMessage(error, copy, copy.mutationError) })
          setRetryIntent({ kind: "create", ...attempt })
        }
        closeDialogState()
      }
      if (isCurrentMutation(generation)) setPending("create", false)
      return
    }
    if (isCurrentMutation(generation)) {
      const reloaded = await reconcileAfterMutation()
      if (!isCurrentMutation(generation)) return
      if (reloaded) {
        optimisticDirtyRef.current = false
        setRetryIntent(null)
        closeDialogState()
      } else {
        optimisticDirtyRef.current = true
        setRetryIntent({ kind: "reload" })
        setNotice({ kind: "stale", message: copy.reconcileStale })
        closeDialogState()
      }
      setPending("create", false)
    }
  }

  const runEdit = async (taskId: string, title: string, retrying = false) => {
    if (surface === undefined || activeModel === null || title.trim().length === 0 || pendingRef.current.has(`edit:${taskId}`)) return
    const generation = mutationGenerationRef.current
    const task = taskForId(activeModel, taskId)
    if (task === null) return
    const snapshot = activeModel
    setPending(`edit:${taskId}`, true)
    optimisticDirtyRef.current = true
    setOptimisticModel(updateTaskOptimistically(snapshot, taskId, title.trim()))
    setNotice(null)
    try {
      await surface.client.updateTask(taskId, { title: title.trim(), expected_lock_version: task.lockVersion })
    } catch (error) {
      if (isCurrentMutation(generation)) {
        setOptimisticModel((current) => current === null ? current : rollbackTaskOptimistically(current, snapshot, taskId))
        if (isMutationConflict(error)) {
          const reloaded = await reconcileAfterMutation()
          if (!isCurrentMutation(generation)) return
          setRetryIntent(reloaded ? { kind: "edit", taskId, title } : { kind: "reload" })
          setNotice({ kind: reloaded ? "conflict" : "stale", message: reloaded ? copy.conflictDescription : copy.reconcileStale })
        } else {
          setNotice({ kind: "error", message: mutationMessage(error, copy, copy.mutationError) })
          if (!retrying) closeDialogState()
        }
        if (pendingRef.current.size <= 1) optimisticDirtyRef.current = false
        setPending(`edit:${taskId}`, false)
      }
      return
    }
    if (isCurrentMutation(generation)) {
      const reloaded = await reconcileAfterMutation()
      if (!isCurrentMutation(generation)) return
      if (reloaded) {
        optimisticDirtyRef.current = false
        setRetryIntent(null)
        closeDialogState()
      } else {
        optimisticDirtyRef.current = true
        setRetryIntent({ kind: "reload" })
        setNotice({ kind: "stale", message: copy.reconcileStale })
        closeDialogState()
      }
      setPending(`edit:${taskId}`, false)
    }
  }

  const runTransition = async (
    taskId: string,
    option: BoardTaskTransitionOption,
    context: { readonly reason?: string; readonly description?: string; readonly confirmed?: boolean } = {},
    retrying = false,
  ) => {
    if (surface === undefined || activeModel === null || pendingRef.current.has(`transition:${taskId}`)) return
    const generation = mutationGenerationRef.current
    const task = taskForId(activeModel, taskId)
    if (task === null) return
    const claimToken = claimTokensRef.current.get(taskId) ?? null
    const legalOption = transitionForTaskTarget(task, option.targetStatus, claimToken)
    if (legalOption === null || legalOption.action !== option.action) {
      if (retrying) {
        setRetryIntent({ kind: "transition", taskId, option, reason: context.reason ?? "", description: context.description ?? "", confirmed: context.confirmed === true })
        setNotice({ kind: "conflict", message: copy.conflictDescription })
      }
      return
    }
    const command = transitionCommandForTask(task, legalOption, { ...context, claimToken })
    if (command === null) {
      setNotice({ kind: "error", message: copy.mutationError })
      return
    }
    const snapshot = activeModel
    setPending(`transition:${taskId}`, true)
    if (legalOption.action !== "unblock") {
      optimisticDirtyRef.current = true
      setOptimisticModel((current) => {
        if (current === null) return current
        const moved = moveTaskOptimistically(current, taskId, legalOption.targetStatus)
        return command.action === "specify" ? updateTaskDescriptionOptimistically(moved, taskId, command.input.description ?? "") : moved
      })
    }
    setNotice(null)
    try {
      const response = await executeBoardTaskTransition(surface.client, taskId, command)
      if (command.action === "claim" && "claim_token" in response.data && typeof response.data.claim_token === "string") {
        claimTokensRef.current.set(taskId, response.data.claim_token)
      } else if (command.action === "submit-review" || command.action === "complete" || command.action === "block" || command.action === "unblock" || command.action === "archive") {
        claimTokensRef.current.delete(taskId)
      }
    } catch (error) {
      if (isCurrentMutation(generation)) {
        setOptimisticModel((current) => current === null ? current : rollbackTaskOptimistically(current, snapshot, taskId))
        if (isMutationConflict(error)) {
          const reloaded = await reconcileAfterMutation()
          if (!isCurrentMutation(generation)) return
          setRetryIntent(reloaded ? { kind: "transition", taskId, option: legalOption, reason: context.reason ?? "", description: context.description ?? "", confirmed: context.confirmed === true } : { kind: "reload" })
          setNotice({ kind: reloaded ? "conflict" : "stale", message: reloaded ? copy.conflictDescription : copy.reconcileStale })
        } else {
          setNotice({ kind: "error", message: mutationMessage(error, copy, copy.mutationError) })
          if (!retrying) closeDialogState()
        }
        if (pendingRef.current.size <= 1) optimisticDirtyRef.current = false
        setPending(`transition:${taskId}`, false)
      }
      return
    }
    if (isCurrentMutation(generation)) {
      const reloaded = await reconcileAfterMutation()
      if (!isCurrentMutation(generation)) return
      if (reloaded) {
        optimisticDirtyRef.current = false
        setRetryIntent(null)
        closeDialogState()
      } else {
        optimisticDirtyRef.current = true
        setRetryIntent({ kind: "reload" })
        setNotice({ kind: "stale", message: copy.reconcileStale })
        closeDialogState()
      }
      setPending(`transition:${taskId}`, false)
    }
  }

  if (activeModel === null || surface === undefined) return null

  const claimTokenForTask = (taskId: string) => claimTokensRef.current.get(taskId) ?? null
  const rememberTrigger = (trigger?: HTMLElement | null) => {
    if (trigger !== undefined) dialogTriggerRef.current = trigger
  }
  const openCreate = (trigger?: HTMLElement | null) => {
    if (pendingRef.current.has("create")) return
    rememberTrigger(trigger)
    setNotice(null)
    const taskId = clientUuid("t_")
    setDialog({ kind: "create", title: "", taskId, idempotencyKey: `task.create:${taskId}` })
  }
  const openEdit = (task: BoardTaskViewModel, trigger?: HTMLElement | null) => {
    if (pendingRef.current.has(`edit:${task.id}`)) return
    rememberTrigger(trigger)
    setNotice(null)
    setDialog({ kind: "edit", taskId: task.id, title: task.title })
  }
  const openTransition = (task: BoardTaskViewModel, option: BoardTaskTransitionOption, trigger?: HTMLElement | null) => {
    if (pendingRef.current.has(`transition:${task.id}`)) return
    const legalOption = transitionForTaskTarget(task, option.targetStatus, claimTokenForTask(task.id))
    if (legalOption === null || legalOption.action !== option.action) return
    rememberTrigger(trigger)
    setNotice(null)
    if (legalOption.requiresReason || legalOption.requiresDescription || legalOption.requiresConfirmation) {
      setDialog({
        kind: "transition",
        taskId: task.id,
        option: legalOption,
        reason: "",
        description: task.description ?? "",
        confirmed: false,
      })
    } else void runTransition(task.id, legalOption)
  }
  const submitDialog = () => {
    if (dialog === null) return
    if (dialog.kind === "create") void runCreate({ title: dialog.title, taskId: dialog.taskId, idempotencyKey: dialog.idempotencyKey })
    else if (dialog.kind === "edit") void runEdit(dialog.taskId, dialog.title)
    else void runTransition(dialog.taskId, dialog.option, {
      reason: dialog.reason,
      description: dialog.description,
      confirmed: dialog.confirmed,
    })
  }
  const closeDialog = () => {
    if (dialog?.kind === "create" && pendingRef.current.has("create")) return
    if (dialog?.kind === "edit" && pendingRef.current.has(`edit:${dialog.taskId}`)) return
    if (dialog?.kind === "transition" && pendingRef.current.has(`transition:${dialog.taskId}`)) return
    closeDialogState()
  }
  const retryMutation = () => {
    const retry = retryIntent
    if (retry === null) return
    if (retry.kind === "reload") void runCanonicalReload()
    else if (retry.kind === "create") void runCreate(retry)
    else if (retry.kind === "edit") void runEdit(retry.taskId, retry.title, true)
    else void runTransition(retry.taskId, retry.option, { reason: retry.reason, description: retry.description, confirmed: retry.confirmed }, true)
  }

  const clearGrab = (announcement = copy.cancelGrab) => {
    const taskId = grabbedTaskId
    dragStateRef.current = null
    setGrabbedTaskId(null)
    if (taskId !== null) taskRefs.current.get(taskId)?.focus()
    setDragAnnouncement(announcement)
  }
  const onDragStart = (taskId: string, event: DragEvent<HTMLElement>) => {
    if (pendingRef.current.has(`transition:${taskId}`)) {
      event.preventDefault()
      return
    }
    const token = typeof globalThis.crypto?.randomUUID === "function" ? globalThis.crypto.randomUUID() : `${Date.now()}-${Math.random()}`
    dragStateRef.current = { taskId, token }
    event.dataTransfer?.setData(internalDragMime, token)
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move"
    setGrabbedTaskId(taskId)
    setDragAnnouncement(copy.grabTask)
  }
  const onDragEnd = (taskId: string) => {
    if (dragStateRef.current?.taskId === taskId || grabbedTaskId === taskId) clearGrab(copy.cancelGrab)
  }
  const currentDragTaskId = (event?: DragEvent<HTMLElement>) => {
    const drag = dragStateRef.current
    if (drag !== null) {
      const token = event?.dataTransfer?.getData(internalDragMime)
      return token === drag.token ? drag.taskId : null
    }
    return grabbedTaskId
  }
  const onDragOver = (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => {
    const taskId = currentDragTaskId(event)
    const task = taskId === null ? null : taskForId(activeModel, taskId)
    const option = task === null ? null : transitionForTaskTarget(task, status, claimTokenForTask(task.id))
    if (option !== null) event.preventDefault()
  }
  const onDrop = (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => {
    const taskId = currentDragTaskId(event)
    event.preventDefault()
    if (taskId === null) {
      setDragAnnouncement(copy.dropRejected)
      return
    }
    const task = taskForId(activeModel, taskId)
    const option = task === null ? null : transitionForTaskTarget(task, status, claimTokenForTask(task.id))
    dragStateRef.current = null
    setGrabbedTaskId(null)
    if (task !== null && option !== null) {
      setDragAnnouncement(copy.dropTask)
      if (option.requiresReason || option.requiresDescription || option.requiresConfirmation) openTransition(task, option, taskRefs.current.get(task.id))
      else void runTransition(task.id, option)
      taskRefs.current.get(task.id)?.focus()
    } else {
      const sameColumn = task?.status === status
      setDragAnnouncement(sameColumn ? copy.dropSameColumn : copy.dropIllegal(task?.status ?? "?", status))
      taskRefs.current.get(taskId)?.focus()
    }
  }
  const onTaskKeyDown = (task: BoardTaskViewModel, event: KeyboardEvent<HTMLElement>) => {
    const target = event.target
    if (target !== event.currentTarget && target instanceof Element && target.closest("button, a, input, textarea, select, [contenteditable='true']") !== null) return
    if (event.key === "Escape" && grabbedTaskId === task.id) {
      event.preventDefault()
      clearGrab()
      return
    }
    if (event.key === " " || event.key === "Spacebar") {
      event.preventDefault()
      if (grabbedTaskId === task.id) clearGrab(copy.releaseTask)
      else {
        setGrabbedTaskId(task.id)
        setDragAnnouncement(copy.grabTask)
      }
      return
    }
    if (grabbedTaskId !== task.id || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) {
      if (event.key === "Enter" && grabbedTaskId === null) {
        event.preventDefault()
        openEdit(task, event.currentTarget)
      }
      return
    }
    event.preventDefault()
    const adjacent = keyboardTransitionForDirection(columns, task.status, event.key === "ArrowRight" ? "next" : "previous")
    const option = adjacent === null ? null : transitionForTaskTarget(task, adjacent.targetStatus, claimTokenForTask(task.id))
    if (option !== null) {
      setGrabbedTaskId(null)
      setDragAnnouncement(copy.dropTask)
      openTransition(task, option, event.currentTarget)
    } else {
      setDragAnnouncement(copy.dropIllegal(task.status, "?"))
    }
  }

  return {
    model: activeModel,
    dialog,
    notice,
    retryIntent,
    grabbedTaskId,
    isPending: (key: string) => pendingKeys.has(key),
    openCreate,
    openEdit,
    openTransition,
    claimTokenForTask,
    setDialogTitle: (title) => setDialog((current) => current === null || current.kind === "transition" ? current : { ...current, title }),
    setDialogReason: (reason) => setDialog((current) => current?.kind === "transition" ? { ...current, reason } : current),
    setDialogDescription: (description) => setDialog((current) => current?.kind === "transition" ? { ...current, description } : current),
    setDialogConfirmed: (confirmed) => setDialog((current) => current?.kind === "transition" ? { ...current, confirmed } : current),
    submitDialog,
    closeDialog,
    retryMutation,
    onDragStart,
    onDragEnd,
    onDragOver,
    onDrop,
    onTaskKeyDown,
    onTaskRef: (taskId, element) => {
      if (element === null) taskRefs.current.delete(taskId)
      else taskRefs.current.set(taskId, element)
    },
    clearGrab,
    dragAnnouncement,
  }
}
