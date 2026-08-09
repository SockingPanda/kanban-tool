import { useEffect, useRef, useState, type DragEvent, type KeyboardEvent } from "react"

import type {
  BoardColumnViewModel,
  BoardMessages,
  BoardTaskViewModel,
  BoardViewModel,
} from "./types"
import {
  isMutationConflict,
  moveTaskOptimistically,
  transitionForTarget,
  updateTaskOptimistically,
  type BoardTaskMutationSurface,
  type BoardTaskTransitionOption,
} from "./task-mutation-state"

export type MutationDialog =
  | { readonly kind: "create"; readonly title: string }
  | { readonly kind: "edit"; readonly taskId: string; readonly title: string }
  | { readonly kind: "transition"; readonly taskId: string; readonly option: BoardTaskTransitionOption; readonly reason: string }

export type RetryIntent =
  | { readonly kind: "create"; readonly title: string }
  | { readonly kind: "edit"; readonly taskId: string; readonly title: string }
  | { readonly kind: "transition"; readonly taskId: string; readonly option: BoardTaskTransitionOption; readonly reason: string }

export interface MutationNotice {
  readonly kind: "error" | "conflict"
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
  readonly openCreate: () => void
  readonly openEdit: (task: BoardTaskViewModel) => void
  readonly openTransition: (task: BoardTaskViewModel, option: BoardTaskTransitionOption) => void
  readonly setDialogTitle: (title: string) => void
  readonly setDialogReason: (reason: string) => void
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

function mutationMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.length > 0 ? error.message : fallback
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
  const mountedRef = useRef(true)
  const boardIdentityRef = useRef<string | null>(baseModel?.board?.id ?? null)
  const mutationGenerationRef = useRef(0)

  useEffect(() => () => {
    mountedRef.current = false
  }, [])

  useEffect(() => {
    const nextBoardIdentity = baseModel?.board?.id ?? null
    if (boardIdentityRef.current === nextBoardIdentity) return
    boardIdentityRef.current = nextBoardIdentity
    mutationGenerationRef.current += 1
    pendingRef.current = new Set()
    setPendingKeys(new Set())
    setOptimisticModel(baseModel)
    setDialog(null)
    setNotice(null)
    setRetryIntent(null)
    setGrabbedTaskId(null)
    setDragAnnouncement(copy.grabTask)
  }, [baseModel, copy.grabTask])

  useEffect(() => {
    if (pendingKeys.size === 0 && baseModel !== null) setOptimisticModel(baseModel)
  }, [baseModel, pendingKeys.size])

  const activeModel = (pendingKeys.size > 0 ? optimisticModel : baseModel) ?? baseModel

  const setPending = (key: string, pending: boolean) => {
    const update = updatePendingMutation(pendingRef.current, key, pending)
    pendingRef.current = update.next
    setPendingKeys(update.next)
  }

  const reloadCanonical = async () => {
    await surface?.onCanonicalReload?.()
  }

  const isCurrentMutation = (generation: number) => mountedRef.current && mutationGenerationRef.current === generation

  const runCreate = async (title: string, retrying = false) => {
    if (surface === undefined || title.trim().length === 0 || pendingRef.current.has("create")) return
    const generation = mutationGenerationRef.current
    setPending("create", true)
    setNotice(null)
    try {
      await surface.client.createTask({ title: title.trim() })
      await reloadCanonical()
      if (isCurrentMutation(generation)) {
        setDialog(null)
        setRetryIntent(null)
      }
    } catch (error) {
      if (!isCurrentMutation(generation)) return
      if (isMutationConflict(error)) {
        setRetryIntent({ kind: "create", title })
        await reloadCanonical().catch(() => undefined)
        setNotice({ kind: "conflict", message: copy.conflictDescription })
      } else {
        setNotice({ kind: "error", message: mutationMessage(error, copy.mutationError) })
        if (!retrying) setDialog(null)
      }
    } finally {
      if (isCurrentMutation(generation)) setPending("create", false)
    }
  }

  const runEdit = async (taskId: string, title: string, retrying = false) => {
    if (surface === undefined || activeModel === null || title.trim().length === 0 || pendingRef.current.has(`edit:${taskId}`)) return
    const generation = mutationGenerationRef.current
    const task = taskForId(activeModel, taskId)
    if (task === null) return
    const snapshot = activeModel
    setPending(`edit:${taskId}`, true)
    setOptimisticModel(updateTaskOptimistically(snapshot, taskId, title.trim()))
    setNotice(null)
    try {
      await surface.client.updateTask(taskId, { title: title.trim(), expected_lock_version: task.lockVersion })
      await reloadCanonical()
      if (isCurrentMutation(generation)) {
        setDialog(null)
        setRetryIntent(null)
      }
    } catch (error) {
      if (!isCurrentMutation(generation)) return
      setOptimisticModel(snapshot)
      if (isMutationConflict(error)) {
        setRetryIntent({ kind: "edit", taskId, title })
        await reloadCanonical().catch(() => undefined)
        setNotice({ kind: "conflict", message: copy.conflictDescription })
      } else {
        setNotice({ kind: "error", message: mutationMessage(error, copy.mutationError) })
        if (!retrying) setDialog(null)
      }
    } finally {
      if (isCurrentMutation(generation)) setPending(`edit:${taskId}`, false)
    }
  }

  const runTransition = async (
    taskId: string,
    option: BoardTaskTransitionOption,
    reason = "",
    retrying = false,
  ) => {
    if (surface === undefined || activeModel === null || pendingRef.current.has(`transition:${taskId}`)) return
    const generation = mutationGenerationRef.current
    if (option.requiresReason && reason.trim().length === 0) return
    const task = taskForId(activeModel, taskId)
    if (task === null) return
    const legalOption = transitionForTarget(task.status, option.targetStatus)
    if (legalOption === null || legalOption.action !== option.action) return
    const snapshot = activeModel
    setPending(`transition:${taskId}`, true)
    setOptimisticModel(moveTaskOptimistically(snapshot, taskId, legalOption.targetStatus))
    setNotice(null)
    try {
      const transition = surface.client.transitionTask as unknown as (
        taskId: string,
        action: BoardTaskTransitionOption["action"],
        input?: Readonly<Record<string, unknown>>,
      ) => Promise<unknown>
      await transition(taskId, legalOption.action, transitionInput(legalOption, reason))
      await reloadCanonical()
      if (isCurrentMutation(generation)) {
        setDialog(null)
        setRetryIntent(null)
      }
    } catch (error) {
      if (!isCurrentMutation(generation)) return
      setOptimisticModel(snapshot)
      if (isMutationConflict(error)) {
        setRetryIntent({ kind: "transition", taskId, option: legalOption, reason })
        await reloadCanonical().catch(() => undefined)
        setNotice({ kind: "conflict", message: copy.conflictDescription })
      } else {
        setNotice({ kind: "error", message: mutationMessage(error, copy.mutationError) })
        if (!retrying) setDialog(null)
      }
    } finally {
      if (isCurrentMutation(generation)) setPending(`transition:${taskId}`, false)
    }
  }

  if (activeModel === null || surface === undefined) return null

  const openCreate = () => {
    if (pendingRef.current.has("create")) return
    setNotice(null)
    setDialog({ kind: "create", title: "" })
  }
  const openEdit = (task: BoardTaskViewModel) => {
    if (pendingRef.current.has(`edit:${task.id}`)) return
    setNotice(null)
    setDialog({ kind: "edit", taskId: task.id, title: task.title })
  }
  const openTransition = (task: BoardTaskViewModel, option: BoardTaskTransitionOption) => {
    if (pendingRef.current.has(`transition:${task.id}`)) return
    setNotice(null)
    if (option.requiresReason) setDialog({ kind: "transition", taskId: task.id, option, reason: "" })
    else void runTransition(task.id, option)
  }
  const submitDialog = () => {
    if (dialog === null) return
    if (dialog.kind === "create") void runCreate(dialog.title)
    else if (dialog.kind === "edit") void runEdit(dialog.taskId, dialog.title)
    else void runTransition(dialog.taskId, dialog.option, dialog.reason)
  }
  const closeDialog = () => {
    if (dialog?.kind === "create" && pendingRef.current.has("create")) return
    if (dialog?.kind === "edit" && pendingRef.current.has(`edit:${dialog.taskId}`)) return
    if (dialog?.kind === "transition" && pendingRef.current.has(`transition:${dialog.taskId}`)) return
    setDialog(null)
  }
  const retryMutation = () => {
    const retry = retryIntent
    if (retry === null) return
    setRetryIntent(null)
    if (retry.kind === "create") void runCreate(retry.title, true)
    else if (retry.kind === "edit") void runEdit(retry.taskId, retry.title, true)
    else void runTransition(retry.taskId, retry.option, retry.reason, true)
  }

  const clearGrab = (announcement = copy.cancelGrab) => {
    const taskId = grabbedTaskId
    setGrabbedTaskId(null)
    if (taskId !== null) taskRefs.current.get(taskId)?.focus()
    setDragAnnouncement(announcement)
  }
  const onDragStart = (taskId: string, event: DragEvent<HTMLElement>) => {
    if (pendingRef.current.has(`transition:${taskId}`)) {
      event.preventDefault()
      return
    }
    event.dataTransfer?.setData("text/plain", taskId)
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move"
    setGrabbedTaskId(taskId)
    setDragAnnouncement(copy.grabTask)
  }
  const onDragEnd = (taskId: string) => {
    if (grabbedTaskId === taskId) clearGrab(copy.cancelGrab)
  }
  const onDragOver = (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => {
    const task = grabbedTaskId === null ? null : taskForId(activeModel, grabbedTaskId)
    const option = task === null ? null : transitionForTarget(task.status, status)
    if (option !== null) event.preventDefault()
  }
  const onDrop = (status: BoardTaskViewModel["status"], event: DragEvent<HTMLElement>) => {
    event.preventDefault()
    const taskId = event.dataTransfer?.getData("text/plain") || grabbedTaskId
    if (!taskId) return
    const task = taskForId(activeModel, taskId)
    const option = task === null ? null : transitionForTarget(task.status, status)
    setGrabbedTaskId(null)
    if (task !== null && option !== null) {
      setDragAnnouncement(copy.dropTask)
      if (option.requiresReason) setDialog({ kind: "transition", taskId: task.id, option, reason: "" })
      else void runTransition(task.id, option)
      taskRefs.current.get(task.id)?.focus()
    } else {
      setDragAnnouncement(copy.cancelGrab)
      taskRefs.current.get(taskId)?.focus()
    }
  }
  const onTaskKeyDown = (task: BoardTaskViewModel, event: KeyboardEvent<HTMLElement>) => {
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
        openEdit(task)
      }
      return
    }
    event.preventDefault()
    const option = keyboardTransitionForDirection(columns, task.status, event.key === "ArrowRight" ? "next" : "previous")
    if (option !== null) {
      setGrabbedTaskId(null)
      setDragAnnouncement(copy.dropTask)
      if (option.requiresReason) setDialog({ kind: "transition", taskId: task.id, option, reason: "" })
      else void runTransition(task.id, option)
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
    setDialogTitle: (title) => setDialog((current) => {
      if (current === null) return current
      return current.kind === "transition" ? current : { ...current, title }
    }),
    setDialogReason: (reason) => setDialog((current) => current?.kind === "transition" ? { ...current, reason } : current),
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
