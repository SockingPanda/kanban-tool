import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode, type SyntheticEvent } from "react"

import type { Locale } from "../../lib/preferences"
import { taskOpenerKey } from "../../lib/explorer-focus"
import styles from "./TaskInspector.module.css"
import {
  buildInspectorSaveTaskInput,
  buildInspectorTransitionCommand,
  inspectorActionViews,
  inspectorEditDraft,
  inspectorMutationCommitted,
  inspectorRetryIntentMatches,
  inspectorRetryUserIntentMatches,
  type InspectorEditDraft,
  type InspectorActionView,
} from "./TaskInspector.edit-actions"
import {
  TaskInspectorActionDialog,
  TaskInspectorActionPanel,
  TaskInspectorEditForm,
  type InspectorActionDialogState,
} from "./TaskInspectorEditActions"
import { createInspectorAsyncFence, type InspectorAsyncFence } from "./TaskInspector.lazy"
import { inspectorMutationKey, type InspectorMutationOutcome, type InspectorTransitionCommand, type TaskInspectorMutationError, type TaskInspectorMutationHandlers, type TaskInspectorMutationSnapshot } from "./task-inspector-mutation-state"

export type InspectorTaskStatus = "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review" | "done" | "archived"
export type InspectorPlanState = "unplanned" | "planned" | "not_required"

export interface TaskInspectorViewModel {
  readonly task: {
    readonly id: string
    readonly ref: string
    readonly title: string
    readonly status: InspectorTaskStatus
    /** UI-only optimistic concurrency inputs; canonical values come from the read mapper. */
    readonly lockVersion?: number
    readonly scheduledAt?: number | null
    readonly dueAt?: number | null
    readonly priority: number
    readonly description: string | null
    readonly statusReason: string | null
    readonly assignee: string | null
    readonly executionPlanState: InspectorPlanState
    readonly dependencyBlocked: boolean
    readonly unfinishedParentCount: number
    readonly requiredStepCount: number
    readonly completedRequiredStepCount: number
    readonly optionalStepCount: number
    readonly metadata: unknown
    readonly claimOwner: string | null
    readonly claimExpiresAt: number | null
    readonly lastHeartbeatAt: number | null
    readonly currentRunId: string | null
    readonly retryCount: number
    readonly maxRetries: number | null
    readonly createdAt: number
    readonly updatedAt: number
  }
  readonly steps: readonly {
    readonly id: string
    readonly title: string
    readonly status: "todo" | "done" | "skipped"
    readonly required: boolean
    readonly body: string | null
  }[]
  readonly parents: readonly InspectorDependency[]
  readonly children: readonly InspectorDependency[]
  readonly comments: readonly {
    readonly id: string
    readonly author: string
    readonly kind: "note" | "decision" | "signal"
    readonly body: string
    readonly createdAt: number
  }[]
  readonly runs: readonly {
    readonly id: string
    readonly status: "running" | "succeeded" | "failed" | "canceled" | "expired"
    readonly workerProfile: string | null
    readonly claimOwner: string
    readonly startedAt: number
    readonly finishedAt: number | null
    readonly exitCode: number | null
    readonly error: string | null
    readonly hasLog: boolean
  }[]
  readonly events: readonly {
    readonly id: number
    readonly kind: string
    readonly actor: string | null
    readonly createdAt: number
  }[]
  readonly neighborhood?: {
    readonly centerTaskId: string
    readonly nodes: readonly { readonly id: string; readonly ref: string; readonly title: string; readonly role: string }[]
    readonly edges: readonly { readonly id: string; readonly sourceTaskId: string; readonly targetTaskId: string; readonly kind: string }[]
  }
  readonly runtime: {
    readonly actor: string
    readonly apiBaseUrl: string
    readonly serverVersion: string
    readonly protocolVersion: string
    readonly webBuildId: string
  }
}

export interface InspectorDependency {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: InspectorTaskStatus
}

export interface TaskInspectorProps {
  readonly model: TaskInspectorViewModel
  readonly onSelectTask: (taskId: string) => void
  readonly locale?: Locale
  /** Runtime/session + task identity used to fence deferred section reads. */
  readonly identity?: string
  readonly refreshRevision?: number
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
  /** Claim token is held by the shared claim-token store and never rendered. */
  readonly claimToken?: string | null
}

export type InspectorCopy = {
  readonly ariaLabel: string
  readonly eyebrow: string
  readonly dependencyBlocked: string
  readonly sections: {
    readonly metadata: string
    readonly claim: string
    readonly steps: string
    readonly dependencies: string
    readonly comments: string
    readonly runs: string
    readonly events: string
    readonly neighborhood: string
    readonly runtime: string
  }
  readonly facts: {
    readonly statusReason: string
    readonly assignee: string
    readonly plan: string
    readonly requiredSteps: string
    readonly optionalSteps: string
    readonly createdAt: string
    readonly updatedAt: string
    readonly claimOwner: string
    readonly claimExpires: string
    readonly heartbeat: string
    readonly currentRun: string
    readonly retry: string
    readonly blockedParents: string
    readonly actor: string
    readonly api: string
    readonly server: string
    readonly protocol: string
    readonly build: string
  }
  readonly parents: string
  readonly children: string
  readonly description: string
  readonly showDescription: string
  readonly noDescription: string
  readonly noItems: string
  readonly noSteps: string
  readonly noComments: string
  readonly noRuns: string
  readonly noEvents: string
  readonly noNeighborhood: string
  readonly neighborhoodNodes: string
  readonly neighborhoodEdges: string
  readonly required: string
  readonly manual: string
  readonly system: string
  readonly log: string
  readonly loading: string
  readonly loadError: string
  readonly retry: string
  readonly openAnnouncement: string
  readonly refreshError: string
  readonly refreshOffline: string
  readonly refreshPending: string
  readonly offline: string
  readonly edit: string
  readonly save: string
  readonly saving: string
  readonly cancel: string
  readonly unsavedChanges: string
  readonly editTitle: string
  readonly editDescription: string
  readonly editAssignee: string
  readonly editPriority: string
  readonly editScheduledAt: string
  readonly editDueAt: string
  readonly actions: string
  readonly actionPending: string
  readonly actionRunning: string
  readonly actionConfirmTitle: string
  readonly actionConfirmDescription: string
  readonly actionDescriptionTitle: string
  readonly actionDescriptionHint: string
  readonly actionReasonTitle: string
  readonly actionReasonHint: string
  readonly actionForceConfirmation: string
  readonly reasonRequired: string
  readonly confirmationRequired: string
  readonly descriptionRequired: string
  readonly retryAction: string
  readonly mutationError: string
  readonly mutationRetrying: string
  readonly actionReasons: {
    readonly description: string
    readonly dependencies: string
    readonly plan: string
    readonly promote: string
    readonly claim: string
    readonly requiredSteps: string
    readonly status: string
  }
  readonly status: Readonly<Record<InspectorTaskStatus, string>>
  readonly planState: Readonly<Record<InspectorPlanState, string>>
  readonly stepStatus: Readonly<Record<"todo" | "done" | "skipped", string>>
  readonly runStatus: Readonly<Record<"running" | "succeeded" | "failed" | "canceled" | "expired", string>>
  readonly commentKind: Readonly<Record<"note" | "decision" | "signal", string>>
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

function valueOrDash(value: string | number | null): string {
  if (value === null || (typeof value === "string" && value.trim().length === 0)) return "—"
  return String(value)
}

function jsonValue(value: unknown): string {
  try {
    return JSON.stringify(value) ?? "{}"
  } catch {
    return "[unserializable metadata]"
  }
}

function Section({ id, title, children }: { readonly id: string; readonly title: string; readonly children: ReactNode }) {
  return (
    <section className={styles.section} id={id} data-testid={id}>
      <h2>{title}</h2>
      {children}
    </section>
  )
}

function Facts({ facts }: { readonly facts: readonly [string, string][] }) {
  return (
    <dl className={styles.facts}>
      {facts.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd translate="no">{value}</dd>
        </div>
      ))}
    </dl>
  )
}

function Empty({ children }: { readonly children: ReactNode }) {
  return <p className={styles.empty} role="status">{children}</p>
}

function lazySectionNotice(status: InspectorSectionStatus, online: boolean, copy: InspectorCopy): string | null {
  if (status === "loading") return copy.loading
  if (status === "error") return copy.loadError
  if (status === "offline") return copy.offline
  if (status === "stale") return online ? copy.refreshPending : copy.offline
  return null
}

function DependencyList({
  title,
  tasks,
  onSelectTask,
  copy,
}: {
  readonly title: string
  readonly tasks: readonly InspectorDependency[]
  readonly onSelectTask: (taskId: string) => void
  readonly copy: InspectorCopy
}) {
  return (
    <div className={styles.dependencyGroup}>
      <h3>{title}</h3>
      {tasks.length === 0 ? <Empty>{copy.noItems}</Empty> : (
        <ul className={styles.compactList}>
          {tasks.map((task) => (
            <li key={task.id}>
              <button type="button" className={styles.linkButton} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>
                <span translate="no">{task.ref}</span>
                <span>{task.title}</span>
                <span className={styles.muted}>{copy.status[task.status]}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

function DescriptionDisclosure({ description, copy }: { readonly description: string | null; readonly copy: InspectorCopy }) {
  const [open, setOpen] = useState(false)
  return (
    <details className={styles.descriptionDisclosure} onToggle={(event: SyntheticEvent<HTMLDetailsElement>) => setOpen(event.currentTarget.open)}>
      <summary>{copy.showDescription}</summary>
      {open ? <p className={styles.description}>{description || copy.noDescription}</p> : null}
    </details>
  )
}

function Neighborhood({ model, copy, onSelectTask }: { readonly model: NonNullable<TaskInspectorViewModel["neighborhood"]>; readonly copy: InspectorCopy; readonly onSelectTask: (taskId: string) => void }) {
  return (
    <div className={styles.neighborhood}>
      <div className={styles.neighborhoodFacts}>
        <span>{copy.neighborhoodNodes}: {model.nodes.length}</span>
        <span>{copy.neighborhoodEdges}: {model.edges.length}</span>
      </div>
      {model.nodes.length === 0 ? <Empty>{copy.noNeighborhood}</Empty> : (
        <ul className={styles.compactList}>
          {model.nodes.map((node) => (
            <li key={node.id}>
              <button type="button" className={styles.linkButton} data-task-opener={taskOpenerKey(node.id)} onClick={() => onSelectTask(node.id)}>
                <span translate="no">{node.ref}</span>
                <span>{node.title}</span>
                <span className={styles.muted}>{node.role}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {model.edges.length > 0 ? <ul className={styles.compactList}>{model.edges.map((edge) => <li key={edge.id} className={styles.row}><span translate="no">{edge.sourceTaskId} → {edge.targetTaskId}</span><span className={styles.muted}>{edge.kind}</span></li>)}</ul> : null}
    </div>
  )
}

type InspectorSectionStatus = "idle" | "loading" | "ready" | "stale" | "offline" | "error"

export function TaskInspector({ model, onSelectTask, locale = "zh", identity, refreshRevision = 0, refreshError, refreshOffline = false, online = true, onRetry, onLoadRuns, onLoadEvents, onLoadNeighborhood, mutationHandlers, mutationSnapshot, claimToken = null }: TaskInspectorProps) {
  const { task } = model
  const copy = copies[locale]
  const requestIdentity = identity ?? task.id
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
  const currentTransitionCommandRef = useRef<InspectorTransitionCommand | null>(null)
  const editDraftRef = useRef(editDraft)
  editDraftRef.current = editDraft
  const taskRef = useRef(task)
  taskRef.current = task
  const canonicalEditorDraftKey = JSON.stringify(inspectorEditDraft(task))
  const canonicalEditorDraftKeyRef = useRef(canonicalEditorDraftKey)
  const runsDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const eventsDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const neighborhoodDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const mountedRef = useRef(false)
  const requestIdentityRef = useRef(requestIdentity)
  const lastRefreshRevisionRef = useRef<number | null>(null)
  const runsFenceRef = useRef<InspectorAsyncFence | null>(null)
  const eventsFenceRef = useRef<InspectorAsyncFence | null>(null)
  const neighborhoodFenceRef = useRef<InspectorAsyncFence | null>(null)
  if (runsFenceRef.current === null) runsFenceRef.current = createInspectorAsyncFence()
  if (eventsFenceRef.current === null) eventsFenceRef.current = createInspectorAsyncFence()
  if (neighborhoodFenceRef.current === null) neighborhoodFenceRef.current = createInspectorAsyncFence()
  const runsFence = runsFenceRef.current
  const eventsFence = eventsFenceRef.current
  const neighborhoodFence = neighborhoodFenceRef.current
  const [runs, setRuns] = useState(model.runs)
  const [events, setEvents] = useState(model.events)
  const [neighborhood, setNeighborhood] = useState(model.neighborhood)
  const [runsStatus, setRunsStatus] = useState<InspectorSectionStatus>(model.runs.length > 0 ? "ready" : "idle")
  const [eventsStatus, setEventsStatus] = useState<InspectorSectionStatus>(model.events.length > 0 ? "ready" : "idle")
  const [neighborhoodStatus, setNeighborhoodStatus] = useState<InspectorSectionStatus>(model.neighborhood ? "ready" : "idle")
  const markedRefreshRevisionRef = useRef<number | null>(null)

  const scopedSnapshot = mutationSnapshot?.scope.taskId === task.id && (identity === undefined || mutationSnapshot.scope.identity === identity)
    ? mutationSnapshot
    : undefined
  const mutationGeneration = scopedSnapshot?.generation ?? null
  const mutationScopeKey = JSON.stringify([requestIdentity, task.id, mutationGeneration])
  const mutationScopeKeyRef = useRef(mutationScopeKey)
  if (mutationScopeKeyRef.current !== mutationScopeKey) {
    mutationScopeKeyRef.current = mutationScopeKey
    mutationEpochRef.current += 1
  }
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

  useEffect(() => {
    const currentTask = taskRef.current
    setEditing(false)
    setEditDraft(inspectorEditDraft(currentTask))
    canonicalEditorDraftKeyRef.current = JSON.stringify(inspectorEditDraft(currentTask))
    setActionDialog(null)
    lastTransitionCommandRef.current = null
    currentTransitionCommandRef.current = null
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

  const runMutation = useCallback(async (operation: "saveTask" | "transition", run: () => Promise<InspectorMutationOutcome>): Promise<InspectorMutationOutcome | null> => {
    const taskId = task.id
    const epoch = mutationEpochRef.current
    const key = inspectorMutationKey(operation, taskId)
    setLocalError(null)
    setLocalPending((current) => new Set(current).add(key))
    try {
      const outcome = await run()
      if (!mountedMutationRef.current || mutationEpochRef.current !== epoch || task.id !== taskId) return null
      if (!inspectorMutationCommitted(outcome)) {
        setLocalMutationError(operation)
        return outcome
      }
      setLocalError(null)
      return outcome
    } catch {
      if (!mountedMutationRef.current || mutationEpochRef.current !== epoch || task.id !== taskId) return null
      setLocalMutationError(operation)
      return null
    } finally {
      if (mountedMutationRef.current && mutationEpochRef.current === epoch && task.id === taskId) setLocalPending((current) => {
        const next = new Set(current)
        next.delete(key)
        return next
      })
    }
  }, [setLocalMutationError, task.id])

  const saveTask = useCallback(async (draft: InspectorEditDraft = editDraft): Promise<InspectorMutationOutcome | null> => {
    if (!mutationHandlers || draft.title.trim().length === 0) return null
    const input = buildInspectorSaveTaskInput(task, draft)
    const run = () => mutationHandlers.saveTask(input)
    return runMutation("saveTask", run)
  }, [editDraft, mutationHandlers, runMutation, task])

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
    void saveTask(editDraft).then((saved) => {
      const currentInput = buildInspectorSaveTaskInput(taskRef.current, editDraftRef.current)
      const submittedIntent = { operation: "saveTask" as const, taskId: task.id, input: submittedInput }
      if (inspectorMutationCommitted(saved) && inspectorRetryUserIntentMatches(submittedIntent, "saveTask", currentInput)) closeEditor()
    })
  }, [closeEditor, editDraft, saveTask, task])

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
    currentTransitionCommandRef.current = null
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
    const view = inspectorActionViews(task, claimToken, copy).find((candidate) => candidate.action === actionDialog.action)
    if (!view) return
    if (actionDialog.kind === "description") {
      if (actionDialog.description.trim().length === 0) return
      const context = { description: actionDialog.description }
      const submittedCommand = buildInspectorTransitionCommand(task, view.action, context, claimToken)
      const outcome = await executeTransition(view, context)
      const submittedIntent = submittedCommand === null ? undefined : { operation: "transition" as const, taskId: task.id, command: submittedCommand }
      if (inspectorMutationCommitted(outcome) && submittedIntent !== undefined && inspectorRetryUserIntentMatches(submittedIntent, "transition", currentTransitionCommandRef.current)) closeActionDialog()
    } else if (actionDialog.kind === "reason") {
      if (actionDialog.reason.trim().length === 0 || (actionDialog.requiresConfirmation && !actionDialog.confirmed)) return
      const context = { reason: actionDialog.reason, confirmed: actionDialog.requiresConfirmation ? true : undefined }
      const submittedCommand = buildInspectorTransitionCommand(task, view.action, context, claimToken)
      const outcome = await executeTransition(view, context)
      const submittedIntent = submittedCommand === null ? undefined : { operation: "transition" as const, taskId: task.id, command: submittedCommand }
      if (inspectorMutationCommitted(outcome) && submittedIntent !== undefined && inspectorRetryUserIntentMatches(submittedIntent, "transition", currentTransitionCommandRef.current)) closeActionDialog()
    } else {
      const context = { confirmed: true }
      const submittedCommand = buildInspectorTransitionCommand(task, view.action, context, claimToken)
      const outcome = await executeTransition(view, context)
      const submittedIntent = submittedCommand === null ? undefined : { operation: "transition" as const, taskId: task.id, command: submittedCommand }
      if (inspectorMutationCommitted(outcome) && submittedIntent !== undefined && inspectorRetryUserIntentMatches(submittedIntent, "transition", currentTransitionCommandRef.current)) closeActionDialog()
    }
  }, [actionDialog, claimToken, closeActionDialog, copy, executeTransition, task])

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
  let currentTransitionRetryCommand: InspectorTransitionCommand | null = null
  if (actionDialog === null && lastTransitionCommandRef.current !== null) {
    currentTransitionRetryCommand = buildInspectorTransitionCommand(task, lastTransitionCommandRef.current.action, {}, claimToken)
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
  currentTransitionCommandRef.current = currentTransitionRetryCommand
  const saveRetryIntent = scopedSnapshot?.retries.get(saveRetryKey)
  const transitionRetryIntent = scopedSnapshot?.retries.get(transitionRetryKey)
  const saveRetryMatches = inspectorRetryIntentMatches(saveRetryIntent, "saveTask", currentSaveRetryInput)
  const transitionRetryMatches = inspectorRetryIntentMatches(transitionRetryIntent, "transition", currentTransitionRetryCommand)
  const retryTransitionAction = transitionRetryMatches && transitionRetryIntent?.operation === "transition" ? transitionRetryIntent.command.action : null
  const retryMutation = useCallback(async (operation: "saveTask" | "transition" | "reload"): Promise<InspectorMutationOutcome | null> => {
    if (!mutationHandlers || !scopedSnapshot) return null
    const taskId = task.id
    const epoch = mutationEpochRef.current
    const key = inspectorMutationKey(operation, taskId)
    const retryIntent = scopedSnapshot.retries.get(key)
    if (retryIntent === undefined) return null
    setLocalError(null)
    try {
      const outcome = await mutationHandlers.retry(key)
      if (!mountedMutationRef.current || mutationEpochRef.current !== epoch || task.id !== taskId) return null
      if (!inspectorMutationCommitted(outcome)) {
        if (operation !== "reload") setLocalMutationError(operation)
        return outcome
      }
      setLocalError(null)
      if (operation === "saveTask") {
        const currentInput = buildInspectorSaveTaskInput(taskRef.current, editDraftRef.current)
        if (editing && inspectorRetryUserIntentMatches(retryIntent, "saveTask", currentInput)) closeEditor()
      } else if (operation === "transition" && inspectorRetryUserIntentMatches(retryIntent, "transition", currentTransitionCommandRef.current)) {
        closeActionDialog()
      }
      return outcome
    } catch {
      if (mountedMutationRef.current && mutationEpochRef.current === epoch && task.id === taskId && operation !== "reload") setLocalMutationError(operation)
      return null
    }
  }, [closeActionDialog, closeEditor, editing, mutationHandlers, scopedSnapshot, setLocalMutationError, task.id])
  const retrySave = saveError && saveRetryIntent ? () => { void retryMutation("saveTask") } : null
  const retryTransition = transitionError && transitionRetryIntent ? () => { void retryMutation("transition") } : null
  const retryReload = reloadError && scopedSnapshot?.retries.has(inspectorMutationKey("reload", task.id)) ? () => { void retryMutation("reload") } : null

  useEffect(() => {
    const identityChanged = requestIdentityRef.current !== requestIdentity
    const modelHasLazyData = model.runs.length > 0 || model.events.length > 0 || model.neighborhood !== undefined
    if (identityChanged || modelHasLazyData) {
      runsFence.abort()
      eventsFence.abort()
      neighborhoodFence.abort()
      setRuns(model.runs)
      setEvents(model.events)
      setNeighborhood(model.neighborhood)
      setRunsStatus(model.runs.length > 0 ? "ready" : "idle")
      setEventsStatus(model.events.length > 0 ? "ready" : "idle")
      setNeighborhoodStatus(model.neighborhood ? "ready" : "idle")
    }
    if (!mountedRef.current || identityChanged) headingRef.current?.focus()
    mountedRef.current = true
    requestIdentityRef.current = requestIdentity
    return () => {
      if (identityChanged || modelHasLazyData) {
        runsFence.abort()
        eventsFence.abort()
        neighborhoodFence.abort()
      }
    }
  }, [eventsFence, model.events, model.neighborhood, model.runs, model.task.id, neighborhoodFence, requestIdentity, runsFence])

  useEffect(() => () => {
    runsFence.abort()
    eventsFence.abort()
    neighborhoodFence.abort()
  }, [eventsFence, neighborhoodFence, runsFence])

  const startRunsLoad = useCallback((force = false) => {
    if ((!force && runsStatus !== "idle" && runsStatus !== "stale") || !onLoadRuns) return
    if (!online) {
      setRunsStatus(runs.length > 0 ? "stale" : "offline")
      return
    }
    const signal = runsFence.begin(requestIdentity)
    setRunsStatus("loading")
    void onLoadRuns(signal).then((value) => {
      if (!runsFence.isCurrent(requestIdentity, signal)) return
      setRuns(value)
      setRunsStatus("ready")
    }, () => {
      if (runsFence.isCurrent(requestIdentity, signal)) setRunsStatus("error")
    })
  }, [onLoadRuns, online, requestIdentity, runs, runsFence, runsStatus])
  const startEventsLoad = useCallback((force = false) => {
    if ((!force && eventsStatus !== "idle" && eventsStatus !== "stale") || !onLoadEvents) return
    if (!online) {
      setEventsStatus(events.length > 0 ? "stale" : "offline")
      return
    }
    const signal = eventsFence.begin(requestIdentity)
    setEventsStatus("loading")
    void onLoadEvents(signal).then((value) => {
      if (!eventsFence.isCurrent(requestIdentity, signal)) return
      setEvents(value)
      setEventsStatus("ready")
    }, () => {
      if (eventsFence.isCurrent(requestIdentity, signal)) setEventsStatus("error")
    })
  }, [events, eventsFence, eventsStatus, onLoadEvents, online, requestIdentity])
  const startNeighborhoodLoad = useCallback((force = false) => {
    if ((!force && neighborhoodStatus !== "idle" && neighborhoodStatus !== "stale") || !onLoadNeighborhood) return
    if (!online) {
      setNeighborhoodStatus(neighborhood !== undefined ? "stale" : "offline")
      return
    }
    const signal = neighborhoodFence.begin(requestIdentity)
    setNeighborhoodStatus("loading")
    void onLoadNeighborhood(signal).then((value) => {
      if (!neighborhoodFence.isCurrent(requestIdentity, signal)) return
      setNeighborhood(value)
      setNeighborhoodStatus("ready")
    }, () => {
      if (neighborhoodFence.isCurrent(requestIdentity, signal)) setNeighborhoodStatus("error")
    })
  }, [neighborhood, neighborhoodFence, neighborhoodStatus, onLoadNeighborhood, online, requestIdentity])
  useEffect(() => {
    if (online) return
    setRunsStatus((status) => status === "ready" ? "stale" : status === "idle" && runs.length === 0 ? "offline" : status)
    setEventsStatus((status) => status === "ready" ? "stale" : status === "idle" && events.length === 0 ? "offline" : status)
    setNeighborhoodStatus((status) => status === "ready" ? "stale" : status === "idle" && neighborhood === undefined ? "offline" : status)
  }, [events.length, neighborhood, online, runs.length])
  useEffect(() => {
    if (refreshRevision === 0 || markedRefreshRevisionRef.current === refreshRevision) return
    markedRefreshRevisionRef.current = refreshRevision
    setRunsStatus((status) => status === "ready" ? "stale" : status)
    setEventsStatus((status) => status === "ready" ? "stale" : status)
    setNeighborhoodStatus((status) => status === "ready" ? "stale" : status)
  }, [refreshRevision])
  useEffect(() => {
    if (refreshRevision === 0 || lastRefreshRevisionRef.current === refreshRevision) return
    lastRefreshRevisionRef.current = refreshRevision
    if (runsDetailsRef.current?.open) startRunsLoad(true)
    if (eventsDetailsRef.current?.open) startEventsLoad(true)
    if (neighborhoodDetailsRef.current?.open) startNeighborhoodLoad(true)
  }, [refreshRevision, startEventsLoad, startNeighborhoodLoad, startRunsLoad])
  const loadRuns = (event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open) {
      runsFence.abort()
      if (runsStatus === "loading") setRunsStatus("idle")
      return
    }
    startRunsLoad()
  }
  const loadEvents = (event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open) {
      eventsFence.abort()
      if (eventsStatus === "loading") setEventsStatus("idle")
      return
    }
    startEventsLoad()
  }
  const loadNeighborhood = (event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open) {
      neighborhoodFence.abort()
      if (neighborhoodStatus === "loading") setNeighborhoodStatus("idle")
      return
    }
    startNeighborhoodLoad()
  }
  return (
    <aside className={styles.inspector} data-testid="task-inspector" aria-label={copy.ariaLabel}>
      <header className={styles.header}>
        <p className={styles.eyebrow}>{copy.eyebrow}</p>
        <p className={styles.ref} translate="no">{task.ref}</p>
        <h2 ref={headingRef} tabIndex={-1}>{task.title}</h2>
        <p aria-live="polite" className={styles.announcement}>{copy.openAnnouncement}</p>
        <p className={styles.identity} translate="no">{task.id}</p>
        {refreshError ? <div role={refreshOffline ? "status" : "alert"}><strong>{refreshOffline ? copy.refreshOffline : copy.refreshError}</strong>{!refreshOffline ? <span> {refreshError}</span> : null}{onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}</div> : null}
        {reloadPending ? <div className={styles.mutationError} role="status" aria-live="polite">{copy.mutationRetrying}</div> : null}
        {reloadError ? <div className={styles.mutationError} role="status" aria-live="polite"><span>{reloadError.message}</span>{retryReload ? <button type="button" onClick={retryReload}>{copy.retryAction}</button> : null}</div> : null}
        {!editing && saveError ? <div className={styles.mutationError} role="alert" aria-live="polite"><span>{saveError.message}</span>{retrySave ? <button type="button" onClick={retrySave}>{copy.retryAction}</button> : null}</div> : null}
        {mutationHandlers ? <button ref={editTriggerRef} type="button" className={styles.editButton} onClick={beginEditor} disabled={editing || mutationSavePending}>{copy.edit}</button> : null}
        <div className={styles.badges}>
          <span className={styles.badge}>{copy.status[task.status]}</span>
          <span className={styles.badge}>P{task.priority}</span>
          {task.dependencyBlocked ? <span className={styles.badge}>{copy.dependencyBlocked}</span> : null}
        </div>
      </header>

      {editing && mutationHandlers ? <TaskInspectorEditForm draft={editDraft} dirty={JSON.stringify(editDraft) !== canonicalEditorDraftKey} pending={mutationSavePending} error={saveError?.message ?? null} onRetry={retrySave} retryBlocksSubmit={saveRetryMatches} copy={copy} onChange={setEditDraft} onSave={submitEditor} onCancel={closeEditor} /> : null}
      {mutationHandlers ? <TaskInspectorActionPanel task={task} claimToken={claimToken} locale={locale} copy={copy} pending={mutationTransitionPending} error={transitionError} onAction={openActionDialog} onRetry={retryTransition} retryAction={retryTransitionAction} /> : null}

      <Section id="inspector-metadata" title={copy.sections.metadata}>
        <DescriptionDisclosure description={task.description} copy={copy} />
        <Facts facts={[
          [copy.facts.statusReason, valueOrDash(task.statusReason)],
          [copy.facts.assignee, valueOrDash(task.assignee)],
          [copy.facts.plan, copy.planState[task.executionPlanState]],
          [copy.facts.requiredSteps, `${task.completedRequiredStepCount} / ${task.requiredStepCount}`],
          [copy.facts.optionalSteps, String(task.optionalStepCount)],
          [copy.facts.createdAt, String(task.createdAt)],
          [copy.facts.updatedAt, String(task.updatedAt)],
        ]} />
        <pre className={styles.codeBlock} translate="no">{jsonValue(task.metadata)}</pre>
      </Section>

      <Section id="inspector-claim" title={copy.sections.claim}>
        <Facts facts={[
          [copy.facts.claimOwner, valueOrDash(task.claimOwner)],
          [copy.facts.claimExpires, valueOrDash(task.claimExpiresAt)],
          [copy.facts.heartbeat, valueOrDash(task.lastHeartbeatAt)],
          [copy.facts.currentRun, valueOrDash(task.currentRunId)],
          [copy.facts.retry, `${task.retryCount} / ${valueOrDash(task.maxRetries)}`],
          [copy.facts.blockedParents, String(task.unfinishedParentCount)],
        ]} />
      </Section>

      <Section id="inspector-steps" title={copy.sections.steps}>
        {model.steps.length === 0 ? <Empty>{copy.noSteps}</Empty> : (
          <ol className={styles.compactList}>
            {model.steps.map((step) => (
              <li key={step.id} className={styles.row}>
                <div>
                  <strong>{step.title}</strong>
                  {step.body ? <p className={styles.muted}>{step.body}</p> : null}
                </div>
                <span className={styles.badge}>{copy.stepStatus[step.status]}{step.required ? ` · ${copy.required}` : ""}</span>
              </li>
            ))}
          </ol>
        )}
      </Section>

      <Section id="inspector-dependencies" title={copy.sections.dependencies}>
        <DependencyList title={copy.parents} tasks={model.parents} onSelectTask={onSelectTask} copy={copy} />
        <details>
          <summary>{copy.children}</summary>
          <DependencyList title={copy.children} tasks={model.children} onSelectTask={onSelectTask} copy={copy} />
        </details>
      </Section>

      <Section id="inspector-comments" title={copy.sections.comments}>
        {model.comments.length === 0 ? <Empty>{copy.noComments}</Empty> : (
          <ul className={styles.compactList}>
            {model.comments.map((comment) => (
              <li key={comment.id} className={styles.row}>
                <div><strong>{comment.author}</strong><span className={styles.muted}> · {copy.commentKind[comment.kind]}</span><p>{comment.body}</p></div>
                <time dateTime={String(comment.createdAt)}>{comment.createdAt}</time>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section id="inspector-runs" title={copy.sections.runs}>
        <details ref={runsDetailsRef} onToggle={loadRuns}>
          <summary>{copy.sections.runs}</summary>
          {runs.length === 0 && (runsStatus === "loading" || runsStatus === "error" || runsStatus === "offline") ? <><Empty>{lazySectionNotice(runsStatus, online, copy)}</Empty><button type="button" onClick={() => startRunsLoad(true)}>{copy.retry}</button></> : runs.length === 0 ? <>{runsStatus === "stale" ? <p className={styles.muted} role="status">{copy.refreshPending}</p> : null}<Empty>{copy.noRuns}</Empty></> : (
          <>{lazySectionNotice(runsStatus, online, copy) ? <p className={styles.muted} role={runsStatus === "error" ? "alert" : "status"}>{lazySectionNotice(runsStatus, online, copy)}</p> : null}{runsStatus === "error" || runsStatus === "offline" ? <button type="button" onClick={() => startRunsLoad(true)}>{copy.retry}</button> : null}<ul className={styles.compactList}>
            {runs.map((run) => (
              <li key={run.id} className={styles.row}>
                <div><strong translate="no">{run.id}</strong><span className={styles.muted}> · {copy.runStatus[run.status]}</span><p>{run.workerProfile || copy.manual} · {run.claimOwner}</p>{run.error ? <p className={styles.error}>{run.error}</p> : null}</div>
                <span className={styles.muted}>{run.hasLog ? copy.log : ""}</span>
              </li>
            ))}
          </ul></>
          )}
        </details>
      </Section>

      <Section id="inspector-events" title={copy.sections.events}>
        <details ref={eventsDetailsRef} onToggle={loadEvents}>
          <summary>{copy.sections.events}</summary>
          {events.length === 0 && (eventsStatus === "loading" || eventsStatus === "error" || eventsStatus === "offline") ? <><Empty>{lazySectionNotice(eventsStatus, online, copy)}</Empty><button type="button" onClick={() => startEventsLoad(true)}>{copy.retry}</button></> : events.length === 0 ? <>{eventsStatus === "stale" ? <p className={styles.muted} role="status">{copy.refreshPending}</p> : null}<Empty>{copy.noEvents}</Empty></> : (
          <>{lazySectionNotice(eventsStatus, online, copy) ? <p className={styles.muted} role={eventsStatus === "error" ? "alert" : "status"}>{lazySectionNotice(eventsStatus, online, copy)}</p> : null}{eventsStatus === "error" || eventsStatus === "offline" ? <button type="button" onClick={() => startEventsLoad(true)}>{copy.retry}</button> : null}<ol className={styles.compactList}>
            {events.map((event) => (
              <li key={event.id} className={styles.row}>
                <div><strong translate="no">{event.kind}</strong><p className={styles.muted}>{event.actor || copy.system}</p></div>
                <time dateTime={String(event.createdAt)}>{event.createdAt}</time>
              </li>
            ))}
          </ol></>
          )}
        </details>
      </Section>

      <Section id="inspector-neighborhood" title={copy.sections.neighborhood}>
        <details ref={neighborhoodDetailsRef} onToggle={loadNeighborhood}>
          <summary>{copy.sections.neighborhood}</summary>
          {!neighborhood && (neighborhoodStatus === "loading" || neighborhoodStatus === "error" || neighborhoodStatus === "offline") ? <><Empty>{lazySectionNotice(neighborhoodStatus, online, copy)}</Empty><button type="button" onClick={() => startNeighborhoodLoad(true)}>{copy.retry}</button></> : neighborhood ? <>{lazySectionNotice(neighborhoodStatus, online, copy) ? <p className={styles.muted} role={neighborhoodStatus === "error" ? "alert" : "status"}>{lazySectionNotice(neighborhoodStatus, online, copy)}</p> : null}{neighborhoodStatus === "error" || neighborhoodStatus === "offline" ? <button type="button" onClick={() => startNeighborhoodLoad(true)}>{copy.retry}</button> : null}<Neighborhood model={neighborhood} copy={copy} onSelectTask={onSelectTask} /></> : <>{neighborhoodStatus === "stale" ? <p className={styles.muted} role="status">{copy.refreshPending}</p> : null}<Empty>{copy.noNeighborhood}</Empty></>}
        </details>
      </Section>

      <Section id="inspector-runtime" title={copy.sections.runtime}>
        <Facts facts={[
          [copy.facts.actor, model.runtime.actor],
          [copy.facts.api, model.runtime.apiBaseUrl || "/"],
          [copy.facts.server, model.runtime.serverVersion],
          [copy.facts.protocol, model.runtime.protocolVersion],
          [copy.facts.build, model.runtime.webBuildId],
        ]} />
      </Section>
      {actionDialog ? <TaskInspectorActionDialog dialog={actionDialog} locale={locale} copy={copy} pending={mutationTransitionPending} error={transitionError?.message ?? null} onRetry={retryTransition} retryBlocksSubmit={transitionRetryMatches} onDescriptionChange={(description) => setActionDialog((current) => current?.kind === "description" ? { ...current, description } : current)} onReasonChange={(reason) => setActionDialog((current) => current?.kind === "reason" ? { ...current, reason } : current)} onConfirmationChange={(confirmed) => setActionDialog((current) => current?.kind === "reason" ? { ...current, confirmed } : current)} onCancel={closeActionDialog} onSubmit={submitActionDialog} /> : null}
    </aside>
  )
}
