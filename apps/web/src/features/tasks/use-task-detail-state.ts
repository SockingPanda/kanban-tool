import type { TaskInspectorViewModel, InspectorCopy } from './inspector-model';

import { useCallback, useEffect, useLayoutEffect, useRef, useState, type FormEvent, type ReactNode, type SyntheticEvent } from "react"

import type { Locale } from "../../platform/preferences/preferences"





import {
  buildInspectorSaveTaskInput,
  buildInspectorTransitionCommand,
  inspectorActionViews,
  inspectorActionDialogMatchesTransitionIntent,
  inspectorActionDialogUserIntent,
  inspectorActionDialogUserIntentMatches,
  inspectorEditDraft,
  inspectorMutationCommitted,
  inspectorRetryIntentMatches,
  inspectorRetryUserIntentMatches,
  inspectorTransitionUserIntent,
  type InspectorActionDialogUserIntent,
  type InspectorEditDraft,
  type InspectorActionView,
} from "./TaskInspector.edit-actions"

import { type InspectorActionDialogState } from "./TaskInspectorEditActions";

import { useDeferredRead } from "../../application/query/use-deferred-read";

import { inspectorMutationKey, type InspectorMutationOutcome, type InspectorTransitionCommand, type TaskInspectorMutationError, type TaskInspectorMutationHandlers, type TaskInspectorMutationSnapshot } from "../../application/tasks/task-inspector-mutation-state"

export interface TaskInspectorProps {
  readonly model: TaskInspectorViewModel
  readonly onSelectTask: (taskId: string) => void
  readonly locale?: Locale
  /** Runtime/session + task identity used to fence deferred section reads. */
  readonly identity: string
  readonly refreshError?: string | null
  readonly refreshOffline?: boolean
  readonly online?: boolean
  readonly onRetry?: () => void
  readonly onLoadRuns?: (signal: AbortSignal) => Promise<TaskInspectorViewModel["runs"]>
  readonly onLoadEvents?: (signal: AbortSignal) => Promise<TaskInspectorViewModel["events"]>
  readonly onLoadNeighborhood?: (signal: AbortSignal) => Promise<NonNullable<TaskInspectorViewModel["neighborhood"]>>
  /** Mutation handlers supplied by the shared Explorer mutation controller. */
  readonly mutationHandlers?: TaskInspectorMutationHandlers
  /** Scoped pending/error/retry snapshot from the shared mutation controller. */
  readonly mutationSnapshot?: TaskInspectorMutationSnapshot
  /** Hide the legacy read-only relation sections only while the mutation owner is mounted. */
  readonly dependencyContent?: ReactNode
  readonly hideReadOnlyRelations?: boolean
  /** Claim token is held by the shared claim-token store and never rendered. */
  readonly claimToken?: string | null
}

const copies: Record<Locale, InspectorCopy> = {
  zh: {
    ariaLabel: "任务检查器",
    eyebrow: "任务检查器",
    dependencyBlocked: "依赖阻塞",
    sections: { metadata: "元数据", claim: "运行时 / 认领", steps: "步骤", dependencies: "依赖", comments: "评论", runs: "运行记录", events: "事件", neighborhood: "邻域 / 关系图", runtime: "运行时" },
    facts: { statusReason: "状态原因", assignee: "执行者", plan: "执行计划", requiredSteps: "必需步骤", optionalSteps: "可选步骤", createdAt: "创建时间", updatedAt: "更新时间", claimOwner: "认领者", claimExpires: "认领到期", heartbeat: "最近心跳", currentRun: "当前运行", retry: "重试", blockedParents: "阻塞父任务", actor: "执行者", api: "API", server: "服务版本", protocol: "协议版本", build: "Web 构建" },
    parents: "父任务",
    children: "子任务",
    description: "描述",
    showDescription: "展开描述",
    noDescription: "暂无描述。",
    noItems: "无",
    noSteps: "暂无步骤。",
    noComments: "暂无评论。",
    noRuns: "暂无运行记录。",
    noEvents: "暂无事件。",
    noNeighborhood: "暂无邻域数据。",
    neighborhoodNodes: "节点",
    neighborhoodEdges: "边",
    required: "必需",
    manual: "手动运行",
    system: "系统",
    log: "日志",
    loading: "正在加载…",
    loadError: "加载失败，请重试。",
    retry: "重试",
    openAnnouncement: "已打开任务检查器。",
    refreshError: "任务数据刷新失败。",
    refreshOffline: "当前离线，保留最近一次任务数据。",
    refreshPending: "数据已更新，展开后自动刷新。",
    offline: "当前离线，保留旧数据；联网后重试。",
    edit: "编辑任务",
    save: "保存",
    saving: "正在保存…",
    cancel: "取消",
    unsavedChanges: "有未保存的更改",
    editTitle: "任务标题",
    editDescription: "任务描述",
    editAssignee: "执行者",
    editPriority: "优先级",
    editScheduledAt: "计划时间",
    editDueAt: "截止时间",
    actions: "可用操作",
    actionPending: "正在执行…",
    actionRunning: "正在执行",
    actionConfirmTitle: "确认操作",
    actionConfirmDescription: "此操作会改变任务状态。确认继续吗？",
    actionDescriptionTitle: "指定任务",
    actionDescriptionHint: "指定需要非空描述。",
    actionReasonTitle: "阻塞任务",
    actionReasonHint: "请说明任务为什么无法继续。",
    actionForceConfirmation: "当前没有认领令牌；确认后将强制阻塞任务。",
    reasonRequired: "请填写原因。",
    confirmationRequired: "请确认强制操作。",
    descriptionRequired: "请填写描述。",
    retryAction: "重试操作",
    mutationError: "操作失败，请检查提示后重试。",
    mutationRetrying: "正在重试操作…",
    actionReasons: { description: "需要任务描述", dependencies: "依赖仍未满足", plan: "请先完成执行计划", promote: "规格、排期或就绪条件未满足", claim: "需要当前认领令牌", requiredSteps: "必需步骤尚未完成", status: "当前状态不允许此操作" },
    status: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
    stepStatus: { todo: "待办", done: "已完成", skipped: "已跳过" },
    runStatus: { running: "运行中", succeeded: "成功", failed: "失败", canceled: "已取消", expired: "已过期" },
    commentKind: { note: "备注", decision: "决策", signal: "信号" },
  },
  en: {
    ariaLabel: "Task Inspector",
    eyebrow: "TASK INSPECTOR",
    dependencyBlocked: "Blocked by dependencies",
    sections: { metadata: "Metadata", claim: "Runtime / Claim", steps: "Steps", dependencies: "Dependencies", comments: "Comments", runs: "Runs", events: "Events", neighborhood: "Neighborhood / Map", runtime: "Runtime" },
    facts: { statusReason: "Status reason", assignee: "Assignee", plan: "Execution plan", requiredSteps: "Required steps", optionalSteps: "Optional steps", createdAt: "Created", updatedAt: "Updated", claimOwner: "Claim owner", claimExpires: "Claim expires", heartbeat: "Last heartbeat", currentRun: "Current run", retry: "Retry", blockedParents: "Blocked parents", actor: "Actor", api: "API", server: "Server version", protocol: "Protocol version", build: "Web build" },
    parents: "Parents",
    children: "Children",
    description: "Description",
    showDescription: "Show description",
    noDescription: "No description.",
    noItems: "None",
    noSteps: "No steps.",
    noComments: "No comments.",
    noRuns: "No runs.",
    noEvents: "No events.",
    noNeighborhood: "No neighborhood data.",
    neighborhoodNodes: "Nodes",
    neighborhoodEdges: "Edges",
    required: "required",
    manual: "Manual",
    system: "system",
    log: "log",
    loading: "Loading…",
    loadError: "Failed to load. Try again.",
    retry: "Retry",
    openAnnouncement: "Task Inspector opened.",
    refreshError: "Task data refresh failed.",
    refreshOffline: "You are offline; the last usable task data is retained.",
    refreshPending: "New data is available; this section will refresh when opened.",
    offline: "You are offline; the old value is retained. Retry when connected.",
    edit: "Edit task",
    save: "Save",
    saving: "Saving…",
    cancel: "Cancel",
    unsavedChanges: "Unsaved changes",
    editTitle: "Task title",
    editDescription: "Task description",
    editAssignee: "Assignee",
    editPriority: "Priority",
    editScheduledAt: "Scheduled at",
    editDueAt: "Due at",
    actions: "Available actions",
    actionPending: "Working…",
    actionRunning: "Working",
    actionConfirmTitle: "Confirm action",
    actionConfirmDescription: "This action changes the task state. Continue?",
    actionDescriptionTitle: "Specify task",
    actionDescriptionHint: "Specify requires a non-empty description.",
    actionReasonTitle: "Block task",
    actionReasonHint: "Explain why work cannot continue.",
    actionForceConfirmation: "No current claim token is available; confirm to force blocking the task.",
    reasonRequired: "Enter a reason.",
    confirmationRequired: "Confirm the force action.",
    descriptionRequired: "Enter a description.",
    retryAction: "Retry action",
    mutationError: "Action failed. Review the message and try again.",
    mutationRetrying: "Retrying operation…",
    actionReasons: { description: "Task description is required", dependencies: "Dependencies are still blocked", plan: "Complete the execution plan first", promote: "Specification, schedule, or readiness is incomplete", claim: "A current claim token is required", requiredSteps: "Required steps are incomplete", status: "The current status does not allow this action" },
    status: { triage: "Triage", todo: "To do", scheduled: "Scheduled", ready: "Ready", running: "Running", blocked: "Blocked", review: "Review", done: "Done", archived: "Archived" },
    planState: { unplanned: "Unplanned", planned: "Planned", not_required: "Not required" },
    stepStatus: { todo: "To do", done: "Done", skipped: "Skipped" },
    runStatus: { running: "Running", succeeded: "Succeeded", failed: "Failed", canceled: "Canceled", expired: "Expired" },
    commentKind: { note: "Note", decision: "Decision", signal: "Signal" },
  },
}

interface PendingActionDialogSubmission {
  readonly epoch: number
  readonly taskId: string
  readonly intent: InspectorActionDialogUserIntent
  status: "pending" | "preserve"
}

function retryCommand(task: TaskInspectorViewModel['task'], claimToken: string|null, actionDialog: InspectorActionDialogState|null, actionDialogView: InspectorActionView|null, lastCommand: InspectorTransitionCommand|null) {
  let currentTransitionRetryCommand: InspectorTransitionCommand | null = null
  if (actionDialog === null && lastCommand !== null) {
    currentTransitionRetryCommand = buildInspectorTransitionCommand(task, lastCommand.action, {}, claimToken)
  }
  if (actionDialog !== null && actionDialogView !== null) {
    if (actionDialog.kind === "description") {
      currentTransitionRetryCommand = buildInspectorTransitionCommand(task, actionDialogView.action, { description: actionDialog.description }, claimToken)
    } else if (actionDialog.kind === "reason") {
      currentTransitionRetryCommand = actionDialog.requiresConfirmation && !actionDialog.confirmed
        ? null
        : buildInspectorTransitionCommand(task, actionDialogView.action, { reason: actionDialog.reason, confirmed: actionDialog.requiresConfirmation ? true : undefined }, claimToken)
    } else {
      currentTransitionRetryCommand = buildInspectorTransitionCommand(task, actionDialogView.action, { confirmed: true }, claimToken)
    }
  }
  return currentTransitionRetryCommand;
}

export function useTaskInspectorState({ model, onSelectTask, locale = "zh", identity, refreshError, refreshOffline = false, online = true, onRetry, onLoadRuns, onLoadEvents, onLoadNeighborhood, mutationHandlers, mutationSnapshot, hideReadOnlyRelations = false, dependencyContent, claimToken = null }: TaskInspectorProps) {
  const { task } = model
  const copy = copies[locale]
  const requestIdentity = identity
  const headingRef = useRef<HTMLHeadingElement | null>(null)
  const editTriggerRef = useRef<HTMLButtonElement | null>(null)
  const mountedMutationRef = useRef(true)
  const [editing, setEditing] = useState(false)
  const [editDraft, setEditDraft] = useState(() => inspectorEditDraft(task))
  const [localPending, setLocalPending] = useState<ReadonlySet<string>>(() => new Set())
  const [localError, setLocalError] = useState<TaskInspectorMutationError | null>(null)
  const [actionDialog, setActionDialog] = useState<InspectorActionDialogState | null>(null)
  const mutationEpochRef = useRef(0)
  const lastTransitionCommandRef = useRef<InspectorTransitionCommand | null>(null)
  const actionDialogSubmissionRef = useRef<PendingActionDialogSubmission | null>(null)
  const editDraftRef = useRef(editDraft)
  const taskRef = useRef(task)
  const actionDialogRef = useRef(actionDialog)
  useLayoutEffect(() => {
    editDraftRef.current = editDraft
    taskRef.current = task
    actionDialogRef.current = actionDialog
  }, [actionDialog, editDraft, task])
  const canonicalEditorDraftKey = JSON.stringify(inspectorEditDraft(task))
  const canonicalEditorDraftKeyRef = useRef(canonicalEditorDraftKey)
  const runsDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const eventsDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const neighborhoodDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const runsRead = useDeferredRead(requestIdentity, model.runs, model.runs.length > 0, onLoadRuns, online)
  const eventsRead = useDeferredRead(requestIdentity, model.events, model.events.length > 0, onLoadEvents, online)
  const neighborhoodRead = useDeferredRead(requestIdentity, model.neighborhood, model.neighborhood !== undefined, onLoadNeighborhood, online)
  const { data: runs, status: runsStatus, retry: startRunsLoad } = runsRead
  const { data: events, status: eventsStatus, retry: startEventsLoad } = eventsRead
  const { data: neighborhood, status: neighborhoodStatus, retry: startNeighborhoodLoad } = neighborhoodRead
  const loadRuns = (event: SyntheticEvent<HTMLDetailsElement>) => runsRead.setOpen(event.currentTarget.open)
  const loadEvents = (event: SyntheticEvent<HTMLDetailsElement>) => eventsRead.setOpen(event.currentTarget.open)
  const loadNeighborhood = (event: SyntheticEvent<HTMLDetailsElement>) => neighborhoodRead.setOpen(event.currentTarget.open)
  useEffect(() => { headingRef.current?.focus() }, [requestIdentity])

  const scopedSnapshot = mutationSnapshot?.scope.taskId === task.id && mutationSnapshot.scope.identity === identity
    ? mutationSnapshot
    : undefined
  const mutationGeneration = scopedSnapshot?.generation ?? null
  const mutationScopeKey = JSON.stringify([requestIdentity, task.id, mutationGeneration])
  const mutationScopeKeyRef = useRef(mutationScopeKey)
  useLayoutEffect(() => {
    if (mutationScopeKeyRef.current === mutationScopeKey) return
    mutationScopeKeyRef.current = mutationScopeKey
    mutationEpochRef.current += 1
  }, [mutationScopeKey])
  const mutationPending = useCallback((operation: "saveTask" | "transition") => {
    const key = inspectorMutationKey(operation, task.id)
    const reloadKey = inspectorMutationKey("reload", task.id)
    return localPending.has(key) || Boolean(scopedSnapshot?.pending.has(key)) || Boolean(scopedSnapshot?.pending.has(reloadKey))
  }, [localPending, scopedSnapshot, task.id])
  const snapshotError = useCallback((operation: "saveTask" | "transition" | "reload") => {
    const key = inspectorMutationKey(operation, task.id)
    return scopedSnapshot?.errors.get(key) ?? null
  }, [scopedSnapshot, task.id])

  useEffect(() => {
    mountedMutationRef.current = true
    return () => {
      mountedMutationRef.current = false
    }
  }, [])

  useLayoutEffect(() => {
    const currentTask = taskRef.current
    setEditing(false)
    setEditDraft(inspectorEditDraft(currentTask))
    canonicalEditorDraftKeyRef.current = JSON.stringify(inspectorEditDraft(currentTask))
    setActionDialog(null)
    lastTransitionCommandRef.current = null
    actionDialogSubmissionRef.current = null
    setLocalError(null)
    setLocalPending(new Set())
  }, [mutationGeneration, requestIdentity, task.id])

  useEffect(() => {
    if (editing || canonicalEditorDraftKeyRef.current === canonicalEditorDraftKey) return
    canonicalEditorDraftKeyRef.current = canonicalEditorDraftKey
    setEditDraft(inspectorEditDraft(task))
  }, [canonicalEditorDraftKey, editing, task])

  const setLocalMutationError = useCallback((operation: "saveTask" | "transition") => {
    setLocalError({ operation, taskId: task.id, kind: "error", message: copy.mutationError, status: null, code: null, recoverable: true })
  }, [copy.mutationError, task.id])

  const mutationScopeCurrent = useCallback(
    (epoch: number, taskId: string): boolean => mountedMutationRef.current && mutationEpochRef.current === epoch && taskRef.current.id === taskId,
    [],
  )

  const runMutation = useCallback(async (operation: "saveTask" | "transition", run: () => Promise<InspectorMutationOutcome>): Promise<InspectorMutationOutcome | null> => {
    const taskId = task.id
    const epoch = mutationEpochRef.current
    const key = inspectorMutationKey(operation, taskId)
    setLocalError(null)
    setLocalPending((current) => new Set(current).add(key))
    try {
      const outcome = await run()
      if (!mutationScopeCurrent(epoch, taskId)) return null
      if (!inspectorMutationCommitted(outcome)) {
        setLocalMutationError(operation)
        return outcome
      }
      setLocalError(null)
      return outcome
    } catch {
      if (!mutationScopeCurrent(epoch, taskId)) return null
      setLocalMutationError(operation)
      return null
    } finally {
      if (mutationScopeCurrent(epoch, taskId)) setLocalPending((current) => {
        const next = new Set(current)
        next.delete(key)
        return next
      })
    }
  }, [mutationScopeCurrent, setLocalMutationError, task.id])

  const saveTask = useCallback(async (draft: InspectorEditDraft = editDraft): Promise<InspectorMutationOutcome | null> => {
    if (!mutationHandlers || draft.title.trim().length === 0) return null
    const input = buildInspectorSaveTaskInput(task, draft)
    if (input === null) { setLocalMutationError("saveTask"); return null }
    const run = () => mutationHandlers.saveTask(input)
    return runMutation("saveTask", run)
  }, [editDraft, mutationHandlers, runMutation, setLocalMutationError, task])

  const beginEditor = useCallback(() => {
    setEditDraft(inspectorEditDraft(task))
    setLocalError(null)
    setEditing(true)
  }, [task])

  const closeEditor = useCallback(() => {
    setEditing(false)
    setLocalError(null)
    queueMicrotask(() => editTriggerRef.current?.focus())
  }, [])

  const submitEditor = useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const submittedInput = buildInspectorSaveTaskInput(task, editDraft)
    if (submittedInput === null) { setLocalMutationError("saveTask"); return }
    void saveTask(editDraft).then((saved) => {
      const currentInput = buildInspectorSaveTaskInput(taskRef.current, editDraftRef.current)
      const submittedIntent = { operation: "saveTask" as const, taskId: task.id, input: submittedInput }
      if (inspectorMutationCommitted(saved) && inspectorRetryUserIntentMatches(submittedIntent, "saveTask", currentInput)) closeEditor()
    })
  }, [closeEditor, editDraft, saveTask, setLocalMutationError, task])

  const executeTransition = useCallback(async (view: InspectorActionView, context: { readonly description?: string; readonly reason?: string; readonly confirmed?: boolean }): Promise<InspectorMutationOutcome | null> => {
    if (!mutationHandlers) return null
    const command = buildInspectorTransitionCommand(task, view.action, context, claimToken)
    if (command === null) {
      if (mountedMutationRef.current) setLocalMutationError("transition")
      return null
    }
    lastTransitionCommandRef.current = command
    const run = () => mutationHandlers.transition(command)
    return runMutation("transition", run)
  }, [claimToken, mutationHandlers, runMutation, setLocalMutationError, task])

  const closeActionDialog = useCallback(() => {
    const trigger = actionDialog?.trigger ?? null
    lastTransitionCommandRef.current = null
    actionDialogSubmissionRef.current = null
    setActionDialog(null)
    queueMicrotask(() => trigger?.focus())
  }, [actionDialog])

  const openActionDialog = useCallback((view: InspectorActionView, trigger: HTMLButtonElement) => {
    if (!mutationHandlers) return
    if (view.action === "specify") {
      setActionDialog({ kind: "description", action: "specify", description: task.description ?? "", trigger })
      return
    }
    if (view.action === "block") {
      setActionDialog({ kind: "reason", action: "block", reason: "", confirmed: false, requiresConfirmation: view.requiresConfirmation, trigger })
      return
    }
    if (view.requiresConfirmation) {
      setActionDialog({ kind: "confirm", action: view.action, trigger })
      return
    }
    executeTransition(view, {})
  }, [executeTransition, mutationHandlers, task.description])

  const submitActionDialog = useCallback(async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (!actionDialog) return
    const submittedDialogIntent = inspectorActionDialogUserIntent(actionDialog)
    const submittedTaskId = task.id
    const submittedEpoch = mutationEpochRef.current
    const submittedTransition: PendingActionDialogSubmission = {
      epoch: submittedEpoch,
      taskId: submittedTaskId,
      intent: submittedDialogIntent,
      status: "pending",
    }
    const closeAfterCommit = (outcome: InspectorMutationOutcome | null) => {
      if (!inspectorMutationCommitted(outcome)) return
      if (!mutationScopeCurrent(submittedEpoch, submittedTaskId)) return
      if (!inspectorActionDialogUserIntentMatches(actionDialogRef.current, submittedDialogIntent)) {
        if (actionDialogRef.current !== null && actionDialogSubmissionRef.current === submittedTransition) submittedTransition.status = "preserve"
        return
      }
      closeActionDialog()
    }
    const runSubmittedTransition = async (
      view: InspectorActionView,
      context: { readonly description?: string; readonly reason?: string; readonly confirmed?: boolean },
    ) => {
      actionDialogSubmissionRef.current = submittedTransition
      try {
        const outcome = await executeTransition(view, context)
        closeAfterCommit(outcome)
      } finally {
        if (actionDialogSubmissionRef.current === submittedTransition && submittedTransition.status === "pending") actionDialogSubmissionRef.current = null
      }
    }
    const view = inspectorActionViews(task, claimToken, copy).find((candidate) => candidate.action === actionDialog.action)
    if (!view) return
    if (actionDialog.kind === "description") {
      if (actionDialog.description.trim().length === 0) return
      const context = { description: actionDialog.description }
      const submittedCommand = buildInspectorTransitionCommand(task, view.action, context, claimToken)
      if (submittedCommand === null) return
      await runSubmittedTransition(view, context)
    } else if (actionDialog.kind === "reason") {
      if (actionDialog.reason.trim().length === 0 || (actionDialog.requiresConfirmation && !actionDialog.confirmed)) return
      const context = { reason: actionDialog.reason, confirmed: actionDialog.requiresConfirmation ? true : undefined }
      const submittedCommand = buildInspectorTransitionCommand(task, view.action, context, claimToken)
      if (submittedCommand === null) return
      await runSubmittedTransition(view, context)
    } else {
      const context = { confirmed: true }
      const submittedCommand = buildInspectorTransitionCommand(task, view.action, context, claimToken)
      if (submittedCommand === null) return
      await runSubmittedTransition(view, context)
    }
  }, [actionDialog, claimToken, closeActionDialog, copy, executeTransition, mutationScopeCurrent, task])

  useEffect(() => {
    if (!actionDialog) return
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault()
        closeActionDialog()
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [actionDialog, closeActionDialog])

  const mutationSavePending = mutationPending("saveTask")
  const mutationTransitionPending = mutationPending("transition")
  const {saveError,transitionError,reloadError,reloadPending,saveRetryIntent,transitionRetryIntent,actionDialogView,saveRetryMatches,transitionRetryMatches,retryTransitionAction}=inspectorRetryState({task,claimToken,copy,editing,editDraft,actionDialog,lastTransitionCommand:lastTransitionCommandRef.current,scopedSnapshot,localError,snapshotError});
  useLayoutEffect(() => {
    // 外部规范刷新使操作失去合法 view 时，关闭尚未提交的陈旧对话框；
    // 自己提交的 transition 由 promise 的 user-intent guard 决定是否关闭。
    if (actionDialog === null || actionDialogView !== null || mutationTransitionPending || actionDialogSubmissionRef.current !== null) return
    closeActionDialog()
  }, [actionDialog, actionDialogView, closeActionDialog, mutationTransitionPending])
  const retryMutation = useCallback(async (operation: "saveTask" | "transition" | "reload"): Promise<InspectorMutationOutcome | null> => {
    if (!mutationHandlers || !scopedSnapshot) return null
    const taskId = task.id
    const epoch = mutationEpochRef.current
    const key = inspectorMutationKey(operation, taskId)
    const retryIntent = scopedSnapshot.retries.get(key)
    if (retryIntent === undefined) return null
    const retryDialogSubmission: PendingActionDialogSubmission | null = operation === "transition"
      && retryIntent.operation === "transition"
      && actionDialogRef.current !== null
      ? { epoch, taskId, intent: inspectorTransitionUserIntent(retryIntent.command), status: "pending" }
      : null
    if (retryDialogSubmission !== null) actionDialogSubmissionRef.current = retryDialogSubmission
    const clearPendingRetryDialogSubmission = () => {
      if (retryDialogSubmission !== null && actionDialogSubmissionRef.current === retryDialogSubmission && retryDialogSubmission.status === "pending") actionDialogSubmissionRef.current = null
    }
    setLocalError(null)
    try {
      const outcome = await mutationHandlers.retry(key)
      if (!mutationScopeCurrent(epoch, taskId)) {
        clearPendingRetryDialogSubmission()
        return null
      }
      if (!inspectorMutationCommitted(outcome)) {
        clearPendingRetryDialogSubmission()
        if (operation !== "reload") setLocalMutationError(operation)
        return outcome
      }
      setLocalError(null)
      if (operation === "saveTask") {
        const currentInput = buildInspectorSaveTaskInput(taskRef.current, editDraftRef.current)
        if (editing && inspectorRetryUserIntentMatches(retryIntent, "saveTask", currentInput)) closeEditor()
      } else if (
        operation === "transition"
        && retryIntent.operation === "transition"
        && inspectorActionDialogMatchesTransitionIntent(actionDialogRef.current, retryIntent.command)
      ) {
        closeActionDialog()
      } else if (operation === "transition" && retryIntent.operation === "transition" && actionDialogRef.current !== null) {
        actionDialogSubmissionRef.current = {
          epoch,
          taskId,
          intent: inspectorTransitionUserIntent(retryIntent.command),
          status: "preserve",
        }
      }
      clearPendingRetryDialogSubmission()
      return outcome
    } catch {
      clearPendingRetryDialogSubmission()
      if (mutationScopeCurrent(epoch, taskId) && operation !== "reload") setLocalMutationError(operation)
      return null
    }
  }, [closeActionDialog, closeEditor, editing, mutationHandlers, mutationScopeCurrent, scopedSnapshot, setLocalMutationError, task.id])
  const retrySave = saveError && saveRetryIntent ? () => { void retryMutation("saveTask") } : null
  const retryTransition = transitionError && transitionRetryIntent ? () => { void retryMutation("transition") } : null
  const retryReload = reloadError && scopedSnapshot?.retries.has(inspectorMutationKey("reload", task.id)) ? () => { void retryMutation("reload") } : null

  return {
    copy,
    task,
    headingRef,
    dependencyContent,
    refreshError,
    refreshOffline,
    onRetry,
    reloadPending,
    reloadError,
    retryReload,
    editing,
    saveError,
    retrySave,
    mutationHandlers,
    editTriggerRef,
    beginEditor,
    mutationSavePending,
    editDraft,
    canonicalEditorDraftKey,
    saveRetryMatches,
    setEditDraft,
    submitEditor,
    closeEditor,
    claimToken,
    locale,
    mutationTransitionPending,
    transitionError,
    openActionDialog,
    retryTransition,
    retryTransitionAction,
    hideReadOnlyRelations,
    model,
    onSelectTask,
    runsDetailsRef,
    loadRuns,
    runs,
    runsStatus,
    online,
    startRunsLoad,
    eventsDetailsRef,
    loadEvents,
    events,
    eventsStatus,
    startEventsLoad,
    neighborhoodDetailsRef,
    loadNeighborhood,
    neighborhood,
    neighborhoodStatus,
    startNeighborhoodLoad,
    actionDialog,
    transitionRetryMatches,
    setActionDialog,
    closeActionDialog,
    submitActionDialog
  };
}

function inspectorRetryState({task,claimToken,copy,editing,editDraft,actionDialog,lastTransitionCommand,scopedSnapshot,localError,snapshotError}:{task:TaskInspectorViewModel['task'];claimToken:string|null;copy:InspectorCopy;editing:boolean;editDraft:InspectorEditDraft;actionDialog:InspectorActionDialogState|null;lastTransitionCommand:InspectorTransitionCommand|null;scopedSnapshot:TaskInspectorMutationSnapshot|undefined;localError:TaskInspectorMutationError|null;snapshotError:(operation:'saveTask'|'transition'|'reload')=>TaskInspectorMutationError|null}) {
  const saveError = snapshotError("saveTask") ?? (localError?.operation === "saveTask" ? localError : null)
  const transitionError = snapshotError("transition") ?? (localError?.operation === "transition" ? localError : null)
  const reloadError = snapshotError("reload")
  const reloadPending = Boolean(scopedSnapshot?.pending.has(inspectorMutationKey("reload", task.id)))
  const saveRetryKey = inspectorMutationKey("saveTask", task.id)
  const transitionRetryKey = inspectorMutationKey("transition", task.id)
  const currentSaveRetryInput = editing ? buildInspectorSaveTaskInput(task, editDraft) : null
  const actionDialogView = actionDialog === null
    ? null
    : inspectorActionViews(task, claimToken, copy).find((candidate) => candidate.action === actionDialog.action) ?? null
  const currentTransitionRetryCommand = retryCommand(task, claimToken, actionDialog, actionDialogView, lastTransitionCommand);
  const saveRetryIntent = scopedSnapshot?.retries.get(saveRetryKey)
  const transitionRetryIntent = scopedSnapshot?.retries.get(transitionRetryKey)
  const saveRetryMatches = inspectorRetryIntentMatches(saveRetryIntent, "saveTask", currentSaveRetryInput)
  const transitionRetryMatches = inspectorRetryIntentMatches(transitionRetryIntent, "transition", currentTransitionRetryCommand)
  const retryTransitionAction = transitionRetryMatches && transitionRetryIntent?.operation === "transition" ? transitionRetryIntent.command.action : null

  return {saveError,transitionError,reloadError,reloadPending,saveRetryIntent,transitionRetryIntent,actionDialogView,saveRetryMatches,transitionRetryMatches,retryTransitionAction};
}
