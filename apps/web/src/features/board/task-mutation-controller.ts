import { useEffect, useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent } from "react"

import type {
  BoardColumnViewModel,
  BoardMessages,
  BoardTaskViewModel,
  BoardViewModel,
} from "./types"
import {
  isMutationConflict,
  isClaimTokenConflict,
  executeBoardTaskTransition,
  moveTaskOptimistically,
  rollbackTaskOptimistically,
  updateTaskDescriptionOptimistically,
  transitionCommandForTask,
  transitionForTaskTarget,
  transitionForTarget,
  updateTaskOptimistically,
  type BoardTaskCanonicalReloadOptions,
  type BoardTaskMutationCommitted,
  type BoardTaskMutationSurface,
  type BoardTaskTransitionOption,
} from "./task-mutation-state"

export type MutationDialog =
  | {
      readonly kind: "create"
      readonly title: string
      readonly description: string
      readonly firstStepTitle: string
      readonly taskId: string
      readonly idempotencyKey: string
      readonly taskCreated: boolean
    }
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
  | { readonly kind: "reload"; readonly mutationKind?: BoardTaskMutationCommitted["kind"] }
  | {
      readonly kind: "create"
      readonly title: string
      readonly description: string
      readonly firstStepTitle: string
      readonly taskId: string
      readonly idempotencyKey: string
      readonly taskCreated: boolean
    }
  | { readonly kind: "edit"; readonly taskId: string; readonly title: string }
  | {
      readonly kind: "transition"
      readonly taskId: string
      readonly option: BoardTaskTransitionOption
      readonly reason: string
      readonly description: string
      readonly confirmed: boolean
    }

/** Rebase a retry intent on values still present in its open dialog. */
export function retryIntentWithCurrentDialog(retry: RetryIntent, dialog: MutationDialog | null): RetryIntent {
  if (retry.kind === "create" && dialog?.kind === "create" && dialog.taskId === retry.taskId) {
    return { ...retry, title: dialog.title, description: dialog.description, firstStepTitle: dialog.firstStepTitle }
  }
  if (retry.kind === "edit" && dialog?.kind === "edit" && dialog.taskId === retry.taskId) {
    return { ...retry, title: dialog.title }
  }
  if (retry.kind === "transition" && dialog?.kind === "transition" && dialog.taskId === retry.taskId) {
    return { ...retry, reason: dialog.reason, description: dialog.description, confirmed: dialog.confirmed }
  }
  return retry
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
  readonly isMutationPending: boolean
  readonly openCreate: (trigger?: HTMLElement | null) => void
  readonly openEdit: (task: BoardTaskViewModel, trigger?: HTMLElement | null) => void
  readonly openTransition: (task: BoardTaskViewModel, option: BoardTaskTransitionOption, trigger?: HTMLElement | null) => void
  readonly claimTokenForTask: (taskId: string) => string | null
  readonly setDialogTitle: (title: string) => void
  readonly setDialogReason: (reason: string) => void
  readonly setDialogDescription: (description: string) => void
  readonly setDialogFirstStepTitle: (title: string) => void
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

/** Mutation commits are observable even when the subsequent canonical read is stale. */
function notifyMutationCommitted(surface: BoardTaskMutationSurface, event: BoardTaskMutationCommitted): void {
  try {
    surface.onMutationCommitted?.(event)
  } catch {
    // An observer must not turn a committed server write into a local retry.
  }
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
  const surfaceIdentityRef = useRef<BoardTaskMutationSurface | undefined>(surface)
  const mutationGenerationRef = useRef(0)
  const optimisticDirtyRef = useRef(false)
  const internalDragMime = "application/x-kanban-task"

  const getClaimToken = (taskId: string): string | null => {
    if (surface?.claimTokens !== undefined) return surface.claimTokens.get(taskId) ?? null
    return claimTokensRef.current.get(taskId) ?? null
  }
  const setClaimToken = (taskId: string, token: string): void => {
    if (surface?.claimTokens !== undefined) surface.claimTokens.set(taskId, token)
    else claimTokensRef.current.set(taskId, token)
  }
  const deleteClaimToken = (taskId: string): void => {
    surface?.claimTokens?.delete(taskId)
    claimTokensRef.current.delete(taskId)
  }

  const renderBoardIdentity = baseModel?.board?.id ?? null
  useEffect(() => {
    const claimTokens = claimTokensRef.current
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      claimTokens.clear()
      dragStateRef.current = null
      optimisticDirtyRef.current = false
    }
  }, [])

  // Identity changes are committed in a layout effect. A render that React
  // abandons must not clear refs/state or invalidate the live mutation fence.
  useLayoutEffect(() => {
    const nextBoardIdentity = baseModel?.board?.id ?? null
    const identityChanged = boardIdentityRef.current !== nextBoardIdentity || surfaceIdentityRef.current !== surface
    if (!identityChanged) return
    boardIdentityRef.current = nextBoardIdentity
    surfaceIdentityRef.current = surface
    mutationGenerationRef.current += 1
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
  }, [baseModel, copy.grabTask, surface])

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

  const reloadCanonical = async (): Promise<BoardViewModel | null> => {
    return (await surface?.onCanonicalReload?.()) ?? null
  }

  const isCurrentMutation = (generation: number) =>
    mountedRef.current
    && mutationGenerationRef.current === generation
    && boardIdentityRef.current === renderBoardIdentity
    && surfaceIdentityRef.current === surface

  const adoptCanonicalModel = (canonical: BoardViewModel | null, generation: number): boolean => {
    if (canonical === null || canonical.board.id !== renderBoardIdentity || !isCurrentMutation(generation)) return false
    setOptimisticModel(canonical)
    // Keep the refreshed canonical snapshot active until the visible reader
    // publishes the same model. This makes an immediate conflict retry use
    // the server's lock_version instead of the still-stale prop snapshot.
    optimisticDirtyRef.current = true
    return true
  }

  const closeDialogState = () => {
    setDialog(null)
    const trigger = dialogTriggerRef.current
    dialogTriggerRef.current = null
    if (trigger !== null) queueMicrotask(() => trigger.focus())
  }

  const reconcileAfterMutation = async (
    mutationKind?: BoardTaskMutationCommitted["kind"],
    generation = mutationGenerationRef.current,
    reason?: BoardTaskCanonicalReloadOptions["reason"],
  ): Promise<boolean> => {
    try {
      const canonical = mutationKind === undefined && reason === undefined
        ? await reloadCanonical()
        : (await surface?.onCanonicalReload?.({ reason: reason ?? "initial", mutationKind })) ?? null
      adoptCanonicalModel(canonical, generation)
      return true
    } catch {
      return false
    }
  }

  const runCanonicalReload = async (mutationKind?: BoardTaskMutationCommitted["kind"]) => {
    if (surface === undefined || pendingRef.current.has("reload")) return
    const generation = mutationGenerationRef.current
    setPending("reload", true)
    try {
      const canonical = (await surface.onCanonicalReload?.({ reason: "retry", mutationKind })) ?? null
      if (isCurrentMutation(generation)) {
        setNotice(null)
        setRetryIntent(null)
        optimisticDirtyRef.current = !adoptCanonicalModel(canonical, generation)
      }
    } catch (error) {
      if (isCurrentMutation(generation)) {
        setNotice({ kind: "stale", message: mutationMessage(error, copy, copy.reconcileStale) })
        setRetryIntent({ kind: "reload", mutationKind })
      }
    } finally {
      if (isCurrentMutation(generation)) setPending("reload", false)
    }
  }

  const runCreate = async (
    attempt: {
      readonly title: string
      readonly description: string
      readonly firstStepTitle: string
      readonly taskId: string
      readonly idempotencyKey: string
      readonly taskCreated: boolean
    },
  ) => {
    if (surface === undefined || attempt.title.trim().length === 0 || pendingRef.current.size > 0) return
    const generation = mutationGenerationRef.current
    setPending("create", true)
    setNotice(null)
    setRetryIntent(null)
    let taskCreated = attempt.taskCreated
    try {
      if (!taskCreated) {
        await surface.client.createTask({
          title: attempt.title.trim(),
          description: attempt.description.trim() || null,
          task_id: attempt.taskId,
          idempotency_key: attempt.idempotencyKey,
        })
        taskCreated = true
        // The task write is the canonical create commit. Notify immediately
        // while the create generation still owns this surface; an optional
        // first-step failure must not hide the committed task or trigger a
        // second navigation on its step-only retry.
        if (!isCurrentMutation(generation)) return
        notifyMutationCommitted(surface, { kind: "create", taskId: attempt.taskId })
      }
      // The first write may resolve after a board/runtime identity switch.
      // Do not issue the optional step mutation through the stale surface.
      if (!isCurrentMutation(generation)) return
      if (attempt.firstStepTitle.trim().length > 0) {
        await surface.client.createStep(attempt.taskId, {
          title: attempt.firstStepTitle.trim(),
          required: true,
          idempotency_key: `${attempt.idempotencyKey}:step`,
        })
      }
    } catch (error) {
      if (isCurrentMutation(generation)) {
        if (taskCreated) {
          const reloaded = await reconcileAfterMutation(undefined, generation)
          if (!isCurrentMutation(generation)) return
          setRetryIntent({ kind: "create", ...attempt, taskCreated: true })
          setNotice({ kind: reloaded ? "error" : "stale", message: reloaded ? mutationMessage(error, copy, copy.mutationError) : copy.reconcileStale })
        } else if (isMutationConflict(error)) {
          const reloaded = await reconcileAfterMutation("create", generation)
          if (!isCurrentMutation(generation)) return
          setRetryIntent(reloaded ? { kind: "create", ...attempt, taskCreated: false } : { kind: "reload", mutationKind: "create" })
          setNotice({ kind: reloaded ? "conflict" : "stale", message: reloaded ? copy.conflictDescription : copy.reconcileStale })
        } else {
          setNotice({ kind: "error", message: mutationMessage(error, copy, copy.mutationError) })
          setRetryIntent({ kind: "create", ...attempt, taskCreated: false })
        }
        closeDialogState()
      }
      if (isCurrentMutation(generation)) setPending("create", false)
      return
    }
    if (isCurrentMutation(generation)) {
      const reloaded = attempt.firstStepTitle.trim().length > 0
        ? await reconcileAfterMutation("create", generation, "step")
        : await reconcileAfterMutation(undefined, generation)
      if (!isCurrentMutation(generation)) return
      if (reloaded) {
        setRetryIntent(null)
        setDragAnnouncement(copy.mutationSuccess)
        closeDialogState()
      } else {
        optimisticDirtyRef.current = true
        setRetryIntent({ kind: "reload", mutationKind: "create" })
        setNotice({ kind: "stale", message: copy.reconcileStale })
        closeDialogState()
      }
      setPending("create", false)
    }
  }

  const runEdit = async (taskId: string, title: string, retrying = false) => {
    if (surface === undefined || activeModel === null || title.trim().length === 0 || pendingRef.current.size > 0) return
    const generation = mutationGenerationRef.current
    const task = taskForId(activeModel, taskId)
    if (task === null) return
    const snapshot = activeModel
    setPending(`edit:${taskId}`, true)
    optimisticDirtyRef.current = true
    setOptimisticModel(updateTaskOptimistically(snapshot, taskId, title.trim()))
    setNotice(null)
    setRetryIntent(null)
    try {
      await surface.client.updateTask(taskId, { title: title.trim(), expected_lock_version: task.lockVersion })
    } catch (error) {
      if (isCurrentMutation(generation)) {
        setOptimisticModel((current) => current === null ? current : rollbackTaskOptimistically(current, snapshot, taskId))
        if (isMutationConflict(error)) {
          if (isClaimTokenConflict(error)) deleteClaimToken(taskId)
          const reloaded = await reconcileAfterMutation("edit", generation)
          if (!isCurrentMutation(generation)) return
          // Clear the synchronous mutation fence before exposing the retry
          // control; React may commit the notice before the remaining state
          // updates in this async branch.
          setPending(`edit:${taskId}`, false)
          setRetryIntent(reloaded ? { kind: "edit", taskId, title } : { kind: "reload", mutationKind: "edit" })
          setNotice({ kind: reloaded ? "conflict" : "stale", message: reloaded ? copy.conflictDescription : copy.reconcileStale })
        } else {
          setNotice({ kind: "error", message: mutationMessage(error, copy, copy.mutationError) })
          setRetryIntent({ kind: "edit", taskId, title })
          if (!retrying) closeDialogState()
        }
        if (!isMutationConflict(error)) {
          if (pendingRef.current.size <= 1) optimisticDirtyRef.current = false
          setPending(`edit:${taskId}`, false)
        }
      }
      return
    }
    if (isCurrentMutation(generation)) {
      notifyMutationCommitted(surface, { kind: "edit", taskId })
      const reloaded = await reconcileAfterMutation(undefined, generation)
      if (!isCurrentMutation(generation)) return
      if (reloaded) {
        setRetryIntent(null)
        setDragAnnouncement(copy.mutationSuccess)
        closeDialogState()
      } else {
        optimisticDirtyRef.current = true
        setRetryIntent({ kind: "reload", mutationKind: "edit" })
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
    if (surface === undefined || activeModel === null || pendingRef.current.size > 0) return
    const generation = mutationGenerationRef.current
    const task = taskForId(activeModel, taskId)
    if (task === null) return
    const claimToken = getClaimToken(taskId)
    const legalOption = transitionForTaskTarget(task, option.targetStatus, claimToken)
    if (legalOption === null || legalOption.action !== option.action) {
      if (retrying) {
        // The canonical task may have moved to a state where the old edge is
        // no longer legal. Do not leave a retry intent that can issue the
        // obsolete command again; the user must choose a fresh action.
        setRetryIntent(null)
        setNotice({ kind: "conflict", message: copy.conflictDescription })
      }
      return
    }
    const command = transitionCommandForTask(task, legalOption, { ...context, claimToken })
    if (command === null) {
      if (retrying && legalOption.requiresConfirmation) {
        setRetryIntent({ kind: "transition", taskId, option: legalOption, reason: context.reason ?? "", description: context.description ?? "", confirmed: false })
        setDialog({ kind: "transition", taskId, option: legalOption, reason: context.reason ?? "", description: context.description ?? task.description ?? "", confirmed: false })
        setNotice(null)
        return
      }
      if (retrying) {
        setRetryIntent(null)
        setNotice({ kind: "conflict", message: copy.conflictDescription })
        return
      }
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
    setRetryIntent(null)
    try {
      const response = await executeBoardTaskTransition(surface.client, taskId, command)
      if (isCurrentMutation(generation)) {
        if (command.action === "claim" && "claim_token" in response.data && typeof response.data.claim_token === "string") {
          setClaimToken(taskId, response.data.claim_token)
        } else if (command.action === "submit-review" || command.action === "complete" || command.action === "block" || command.action === "unblock" || command.action === "archive") {
          deleteClaimToken(taskId)
        }
      }
    } catch (error) {
      if (isCurrentMutation(generation)) {
        setOptimisticModel((current) => current === null ? current : rollbackTaskOptimistically(current, snapshot, taskId))
        if (isMutationConflict(error)) {
          if (isClaimTokenConflict(error)) deleteClaimToken(taskId)
          const reloaded = await reconcileAfterMutation("transition", generation)
          if (!isCurrentMutation(generation)) return
          setRetryIntent(reloaded ? { kind: "transition", taskId, option: legalOption, reason: context.reason ?? "", description: context.description ?? "", confirmed: context.confirmed === true } : { kind: "reload", mutationKind: "transition" })
          setNotice({ kind: reloaded ? "conflict" : "stale", message: reloaded ? copy.conflictDescription : copy.reconcileStale })
        } else {
          setNotice({ kind: "error", message: mutationMessage(error, copy, copy.mutationError) })
          setRetryIntent({ kind: "transition", taskId, option: legalOption, reason: context.reason ?? "", description: context.description ?? "", confirmed: context.confirmed === true })
          if (!retrying) closeDialogState()
        }
        if (!isMutationConflict(error) && pendingRef.current.size <= 1) optimisticDirtyRef.current = false
        setPending(`transition:${taskId}`, false)
      }
      return
    }
    if (isCurrentMutation(generation)) {
      notifyMutationCommitted(surface, { kind: "transition", taskId })
      const reloaded = await reconcileAfterMutation(undefined, generation)
      if (!isCurrentMutation(generation)) return
      if (reloaded) {
        setRetryIntent(null)
        setDragAnnouncement(copy.mutationSuccess)
        closeDialogState()
      } else {
        optimisticDirtyRef.current = true
        setRetryIntent({ kind: "reload", mutationKind: "transition" })
        setNotice({ kind: "stale", message: copy.reconcileStale })
        closeDialogState()
      }
      setPending(`transition:${taskId}`, false)
    }
  }

  if (activeModel === null || surface === undefined) return null

  const claimTokenForTask = (taskId: string) => getClaimToken(taskId)
  const rememberTrigger = (trigger?: HTMLElement | null) => {
    if (trigger !== undefined) dialogTriggerRef.current = trigger
  }
  const openCreate = (trigger?: HTMLElement | null) => {
    if (pendingRef.current.size > 0) return
    rememberTrigger(trigger)
    setNotice(null)
    setRetryIntent(null)
    const taskId = clientUuid("t_")
    setDialog({ kind: "create", title: "", description: "", firstStepTitle: "", taskId, idempotencyKey: `task.create:${taskId}`, taskCreated: false })
  }
  const openEdit = (task: BoardTaskViewModel, trigger?: HTMLElement | null) => {
    if (pendingRef.current.size > 0) return
    rememberTrigger(trigger)
    setNotice(null)
    setRetryIntent(null)
    setDialog({ kind: "edit", taskId: task.id, title: task.title })
  }
  const openTransition = (task: BoardTaskViewModel, option: BoardTaskTransitionOption, trigger?: HTMLElement | null) => {
    if (pendingRef.current.size > 0) return
    const legalOption = transitionForTaskTarget(task, option.targetStatus, claimTokenForTask(task.id))
    if (legalOption === null || legalOption.action !== option.action) return
    rememberTrigger(trigger)
    setNotice(null)
    setRetryIntent(null)
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
    if (dialog.kind === "create") void runCreate(dialog)
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
    setRetryIntent(null)
    closeDialogState()
  }
  const retryMutation = () => {
    const retry = retryIntent
    if (retry === null) return
    if (retry.kind === "reload") void runCanonicalReload(retry.mutationKind)
    else {
      const current = retryIntentWithCurrentDialog(retry, dialog)
      setRetryIntent(current)
      if (current.kind === "create") void runCreate(current)
      else if (current.kind === "edit") void runEdit(current.taskId, current.title, true)
      else if (current.kind === "transition") void runTransition(current.taskId, current.option, { reason: current.reason, description: current.description, confirmed: current.confirmed }, true)
    }
  }

  const clearGrab = (announcement = copy.cancelGrab) => {
    const taskId = grabbedTaskId
    dragStateRef.current = null
    setGrabbedTaskId(null)
    if (taskId !== null) taskRefs.current.get(taskId)?.focus()
    setDragAnnouncement(announcement)
  }
  const onDragStart = (taskId: string, event: DragEvent<HTMLElement>) => {
    if (pendingRef.current.size > 0) {
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
  const activeDragTask = () => {
    const drag = dragStateRef.current
    return drag === null ? null : taskForId(activeModel, drag.taskId)
  }
  const currentDragTaskId = (event?: DragEvent<HTMLElement>) => {
    const drag = dragStateRef.current
    if (drag === null || event === undefined) return null
    const token = event.dataTransfer?.getData(internalDragMime)
    return token === drag.token ? drag.taskId : null
  }
  const onDragOver = (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => {
    if (pendingRef.current.size > 0) return
    // HTML5 dragover runs in protected mode, so getData() is unavailable here.
    // The local transaction is the source of truth for whether this target may
    // accept a drop; the MIME token is checked again once drop exposes data.
    const task = activeDragTask()
    const option = task === null || task.status === status ? null : transitionForTaskTarget(task, status, claimTokenForTask(task.id))
    if (option !== null) event.preventDefault()
  }
  const onDrop = (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => {
    if (pendingRef.current.size > 0) {
      event.preventDefault()
      return
    }
    const taskId = currentDragTaskId(event)
    event.preventDefault()
    if (taskId === null) {
      setDragAnnouncement(copy.dropRejected)
      return
    }
    const task = taskForId(activeModel, taskId)
    const option = task === null || task.status === status ? null : transitionForTaskTarget(task, status, claimTokenForTask(task.id))
    dragStateRef.current = null
    setGrabbedTaskId(null)
    if (task !== null && option !== null) {
      setDragAnnouncement(copy.dropTask)
      if (option.requiresReason || option.requiresDescription || option.requiresConfirmation) openTransition(task, option, taskRefs.current.get(task.id))
      else void runTransition(task.id, option)
      taskRefs.current.get(task.id)?.focus()
    } else {
      const sameColumn = task?.status === status
      const blockedTarget = task?.status === "blocked" && status !== "todo"
      setDragAnnouncement(
        sameColumn
          ? copy.dropSameColumn
          : blockedTarget
            ? copy.dropBlockedOnlyTodo
            : copy.dropIllegal(task?.status ?? "?", status),
      )
      taskRefs.current.get(taskId)?.focus()
    }
  }
  const onTaskKeyDown = (task: BoardTaskViewModel, event: KeyboardEvent<HTMLElement>) => {
    const target = event.target
    if (target !== event.currentTarget && target instanceof Element && target.closest("button, a, input, textarea, select, [contenteditable='true']") !== null) return
    if (pendingRef.current.size > 0) return
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
    isMutationPending: pendingKeys.size > 0,
    openCreate,
    openEdit,
    openTransition,
    claimTokenForTask,
    setDialogTitle: (title) => setDialog((current) => current === null || current.kind === "transition" ? current : { ...current, title }),
    setDialogReason: (reason) => setDialog((current) => current?.kind === "transition" ? { ...current, reason } : current),
    setDialogDescription: (description) => setDialog((current) => current === null || current.kind === "edit" ? current : { ...current, description }),
    setDialogFirstStepTitle: (firstStepTitle) => setDialog((current) => current?.kind === "create" ? { ...current, firstStepTitle } : current),
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
