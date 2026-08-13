import { useCallback, useEffect, useLayoutEffect, useRef, useState, type FormEvent, type ReactNode, type SyntheticEvent } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import type { Locale } from "../../lib/preferences"
import { taskOpenerKey } from "../../lib/explorer-focus"
import { CodeBlock, SafeHStack, SafeVStack } from "../../ui/astryx"
import styles from "./TaskInspector.module.css"
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
    readonly resultSummary: string | null
    readonly result: unknown
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
  /** Explorer owns the canonical URL close path for the mobile sheet. */
  readonly onClose?: () => void
  /** Action label supplied by the owning Explorer surface. */
  readonly closeLabel?: string
  /** The shell chooses the responsive presentation mode. */
  readonly mode?: "side-peek" | "sheet"
  readonly locale?: Locale
  /** Runtime/session + task identity used to fence deferred section reads. */
  readonly identity: string
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
  /** Hide the legacy read-only relation sections only while the mutation owner is mounted. */
  readonly hideReadOnlyRelations?: boolean
  /** Claim token is held by the shared claim-token store and never rendered. */
  readonly claimToken?: string | null
}

export type InspectorCopy = {
  readonly ariaLabel: string
  readonly dependencyBlocked: string
  readonly sections: {
    readonly overview: string
    readonly status: string
    readonly properties: string
    readonly relations: string
    readonly activity: string
    readonly readiness: string
    readonly result: string
    readonly execution: string
    readonly rawMetadata: string
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
    readonly readiness: string
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
    readonly resultSummary: string
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
  readonly showMetadata: string
  readonly showResult: string
  readonly noDescription: string
  readonly noResult: string
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
  readonly conflictDescription: string
  readonly mutationRetrying: string
  readonly copyResult: string
  readonly copiedResult: string
  readonly copyResultError: string
  readonly actionReasons: {
    readonly description: string
    readonly dependencies: string
    readonly plan: string
    readonly promote: string
    readonly claim: string
    readonly release: string
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
    dependencyBlocked: "依赖阻塞",
    sections: { overview: "概览", status: "状态", properties: "属性", relations: "关系", activity: "活动", readiness: "就绪摘要", result: "结果证据", execution: "执行与归属", rawMetadata: "原始元数据", claim: "运行时 / 认领", steps: "步骤", dependencies: "依赖", comments: "评论", runs: "运行记录", events: "事件", neighborhood: "邻域 / 关系图", runtime: "运行时" },
    facts: { statusReason: "状态原因", assignee: "执行者", plan: "执行计划", readiness: "就绪度", requiredSteps: "必需步骤", optionalSteps: "可选步骤", createdAt: "创建时间", updatedAt: "更新时间", claimOwner: "认领者", claimExpires: "认领到期", heartbeat: "最近心跳", currentRun: "当前运行", retry: "重试", blockedParents: "阻塞父任务", resultSummary: "结果摘要", actor: "执行者", api: "API", server: "服务版本", protocol: "协议版本", build: "Web 构建" },
    parents: "父任务",
    children: "子任务",
    description: "描述",
    showDescription: "展开描述",
    showMetadata: "查看原始元数据",
    showResult: "查看完整结果",
    noDescription: "暂无描述。",
    noResult: "无结果",
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
    conflictDescription: "任务已被其他操作更新。已重新读取 canonical 状态，请确认后重试。",
    mutationRetrying: "正在重试操作…",
    copyResult: "复制结果",
    copiedResult: "结果已复制",
    copyResultError: "复制结果失败",
    actionReasons: { description: "需要任务描述", dependencies: "依赖仍未满足", plan: "请先完成执行计划", promote: "规格、排期或就绪条件未满足", claim: "需要当前认领令牌", release: "需要当前任务的本地认领令牌才能释放回就绪。", requiredSteps: "必需步骤尚未完成", status: "当前状态不允许此操作" },
    status: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
    stepStatus: { todo: "待办", done: "已完成", skipped: "已跳过" },
    runStatus: { running: "运行中", succeeded: "成功", failed: "失败", canceled: "已取消", expired: "已过期" },
    commentKind: { note: "备注", decision: "决策", signal: "信号" },
  },
  en: {
    ariaLabel: "Task Inspector",
    dependencyBlocked: "Blocked by dependencies",
    sections: { overview: "Overview", status: "Status", properties: "Properties", relations: "Relations", activity: "Activity", readiness: "Readiness summary", result: "Result evidence", execution: "Execution & ownership", rawMetadata: "Raw metadata", claim: "Runtime / Claim", steps: "Steps", dependencies: "Dependencies", comments: "Comments", runs: "Runs", events: "Events", neighborhood: "Neighborhood / Map", runtime: "Runtime" },
    facts: { statusReason: "Status reason", assignee: "Assignee", plan: "Execution plan", readiness: "Readiness", requiredSteps: "Required steps", optionalSteps: "Optional steps", createdAt: "Created", updatedAt: "Updated", claimOwner: "Claim owner", claimExpires: "Claim expires", heartbeat: "Last heartbeat", currentRun: "Current run", retry: "Retry", blockedParents: "Blocked parents", resultSummary: "Result summary", actor: "Actor", api: "API", server: "Server version", protocol: "Protocol version", build: "Web build" },
    parents: "Parents",
    children: "Children",
    description: "Description",
    showDescription: "Show description",
    showMetadata: "Show raw metadata",
    showResult: "Show full result",
    noDescription: "No description.",
    noResult: "No result",
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
    conflictDescription: "The task changed elsewhere. Canonical state was reloaded; review it before trying again.",
    mutationRetrying: "Retrying operation…",
    copyResult: "Copy result",
    copiedResult: "Result copied",
    copyResultError: "Result copy failed",
    actionReasons: { description: "Task description is required", dependencies: "Dependencies are still blocked", plan: "Complete the execution plan first", promote: "Specification, schedule, or readiness is incomplete", claim: "A current claim token is required", release: "A local claim token for this task is required to return it to ready.", requiredSteps: "Required steps are incomplete", status: "The current status does not allow this action" },
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

function statusBadgeVariant(status: InspectorTaskStatus): "neutral" | "info" | "success" | "warning" | "error" {
  switch (status) {
    case "ready":
    case "done": return "success"
    case "running": return "info"
    case "blocked": return "error"
    case "review": return "warning"
    default: return "neutral"
  }
}

function priorityBadgeVariant(priority: number): "neutral" | "info" | "success" | "warning" | "error" {
  if (priority >= 3) return "error"
  if (priority === 2) return "warning"
  if (priority === 1) return "info"
  return "neutral"
}

function stepStatusBadgeVariant(status: "todo" | "done" | "skipped"): "neutral" | "success" | "warning" {
  if (status === "done") return "success"
  if (status === "skipped") return "warning"
  return "neutral"
}

function jsonValue(value: unknown): string {
  try {
    return JSON.stringify(value) ?? "{}"
  } catch {
    return "[unserializable metadata]"
  }
}

function stableJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableJsonValue)
  if (value !== null && typeof value === "object") {
    const record = value as Record<string, unknown>
    return Object.fromEntries(Object.keys(record).sort().map((key) => [key, stableJsonValue(record[key])]))
  }
  return value
}

function resultValue(value: unknown): string {
  if (typeof value === "string") return value
  try {
    const formatted = JSON.stringify(stableJsonValue(value), null, 2)
    return formatted ?? String(value)
  } catch {
    return "[unserializable result]"
  }
}

function Section({ id, title, children, level = 2 }: { readonly id: string; readonly title: string; readonly children: ReactNode; readonly level?: 2 | 3 }) {
  const Heading = level === 3 ? "h3" : "h2"
  return (
    <section className={styles.section} id={id} data-testid={id}>
      <Heading>{title}</Heading>
      {children}
    </section>
  )
}

function InspectorGroup({ id, title, children }: { readonly id: string; readonly title: string; readonly children: ReactNode }) {
  return (
    <section className={styles.group} id={id} data-testid={id} aria-labelledby={`${id}-heading`}>
      <h2 id={`${id}-heading`}>{title}</h2>
      {children}
    </section>
  )
}

function InspectorDisclosureGroup({ id, title, children }: { readonly id: string; readonly title: string; readonly children: ReactNode }) {
  return (
    <details className={`${styles.group} ${styles.groupDisclosure}`} id={id} data-testid={id}>
      <summary className={styles.groupSummary}>{title}</summary>
      <SafeVStack as="div" gap={3} className={styles.groupContent}>{children}</SafeVStack>
    </details>
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
  const [open, setOpen] = useState(true)
  return (
    <details className={styles.descriptionDisclosure} open={open} onToggle={(event: SyntheticEvent<HTMLDetailsElement>) => setOpen(event.currentTarget.open)}>
      <summary>{copy.showDescription}</summary>
      {open ? <p className={styles.description}>{description || copy.noDescription}</p> : null}
    </details>
  )
}

function MetadataDisclosure({ metadata, copy }: { readonly metadata: unknown; readonly copy: InspectorCopy }) {
  return (
    <details className={styles.metadataDisclosure} data-testid="inspector-metadata-disclosure">
      <summary>{copy.showMetadata}</summary>
      <pre className={styles.codeBlock} translate="no">{jsonValue(metadata)}</pre>
    </details>
  )
}

function ResultDisclosure({ summary, result, copy }: { readonly summary: string | null; readonly result: unknown; readonly copy: InspectorCopy }) {
  const resultCode = result === null ? null : resultValue(result)
  const resultSummary = summary === null || summary.trim().length === 0
    ? resultCode === null ? copy.noResult : "—"
    : summary
  return (
    <>
      <Facts facts={[[copy.facts.resultSummary, resultSummary]]} />
      <details className={styles.metadataDisclosure} data-testid="inspector-result-disclosure">
        <summary>{copy.showResult}</summary>
        {resultCode === null ? <Empty>{copy.noResult}</Empty> : (
          <CodeBlock
            code={resultCode}
            language={typeof result === "string" ? "plaintext" : "json"}
            isWrapped
            container="section"
            maxHeight="evidence"
            label={copy.sections.result}
            copyLabel={copy.copyResult}
            copiedLabel={copy.copiedResult}
            errorLabel={copy.copyResultError}
            data-testid="inspector-result-code"
          />
        )}
      </details>
    </>
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

interface PendingActionDialogSubmission {
  readonly epoch: number
  readonly taskId: string
  readonly intent: InspectorActionDialogUserIntent
  status: "pending" | "preserve"
}

export function TaskInspector({ model, onSelectTask, onClose, closeLabel, mode = "side-peek", locale = "zh", identity, refreshRevision = 0, refreshError, refreshOffline = false, online = true, onRetry, onLoadRuns, onLoadEvents, onLoadNeighborhood, mutationHandlers, mutationSnapshot, hideReadOnlyRelations = false, claimToken = null }: TaskInspectorProps) {
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
  useLayoutEffect(() => {
    // 外部规范刷新使操作失去合法 view 时，关闭尚未提交的陈旧对话框；
    // 自己提交的 transition 由 promise 的 user-intent guard 决定是否关闭。
    if (actionDialog === null || actionDialogView !== null || mutationTransitionPending || actionDialogSubmissionRef.current !== null) return
    closeActionDialog()
  }, [actionDialog, actionDialogView, closeActionDialog, mutationTransitionPending])
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
    // The owning modal dialog chooses the initial focus for a narrow sheet.
    // Desktop side-peek keeps the existing task-heading focus behavior.
    if (mode === "side-peek" && (!mountedRef.current || identityChanged)) headingRef.current?.focus()
    mountedRef.current = true
    requestIdentityRef.current = requestIdentity
    return () => {
      if (identityChanged || modelHasLazyData) {
        runsFence.abort()
        eventsFence.abort()
        neighborhoodFence.abort()
      }
    }
  }, [eventsFence, mode, model.events, model.neighborhood, model.runs, model.task.id, neighborhoodFence, requestIdentity, runsFence])

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
    <aside className={styles.inspector} data-testid="task-inspector" data-mode={mode} aria-label={copy.ariaLabel}>
      <div className={styles.stickyControls}>
        <header className={styles.header}>
          <div className={styles.headerTop}>
            <p className={styles.ref} translate="no">{task.ref}</p>
            {onClose ? <button type="button" className={styles.mobileCloseButton} data-testid="task-inspector-mobile-close" onClick={onClose}>{closeLabel ?? copy.ariaLabel}</button> : null}
          </div>
          <h2 ref={headingRef} tabIndex={-1}>{task.title}</h2>
          <p aria-live="polite" className={styles.announcement}>{copy.openAnnouncement}</p>
          <p className={styles.identity} translate="no">{task.id}</p>
          {refreshError ? <div role={refreshOffline ? "status" : "alert"}><strong>{refreshOffline ? copy.refreshOffline : copy.refreshError}</strong>{!refreshOffline ? <span> {refreshError}</span> : null}{onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}</div> : null}
          {reloadPending ? <div className={styles.mutationError} data-testid="task-inspector-reload-feedback" role="status" aria-live="polite">{copy.mutationRetrying}</div> : null}
          {reloadError ? <div className={styles.mutationError} data-testid="task-inspector-reload-feedback" role="status" aria-live="polite"><span>{reloadError.message}</span>{retryReload ? <button type="button" onClick={retryReload}>{copy.retryAction}</button> : null}</div> : null}
          {!editing && saveError ? <div className={styles.mutationError} role="alert" aria-live="polite"><span>{saveError.message}</span>{retrySave ? <button type="button" onClick={retrySave}>{copy.retryAction}</button> : null}</div> : null}
          {mutationHandlers ? <button ref={editTriggerRef} type="button" className={styles.editButton} onClick={beginEditor} disabled={editing || mutationSavePending}>{copy.edit}</button> : null}
        </header>
        {mutationHandlers ? <div className={styles.actionBar} data-testid="inspector-action-bar"><TaskInspectorActionPanel task={task} claimToken={claimToken} locale={locale} copy={copy} pending={mutationTransitionPending} error={transitionError} onAction={openActionDialog} onRetry={retryTransition} retryAction={retryTransitionAction} /></div> : null}
      </div>

      <div className={styles.inspectorBody}>
        {editing && mutationHandlers ? <TaskInspectorEditForm draft={editDraft} dirty={JSON.stringify(editDraft) !== canonicalEditorDraftKey} pending={mutationSavePending} error={saveError?.message ?? null} onRetry={retrySave} retryBlocksSubmit={saveRetryMatches} copy={copy} onChange={setEditDraft} onSave={submitEditor} onCancel={closeEditor} /> : null}

        <InspectorGroup id="inspector-overview" title={copy.sections.overview}>
          <Section id="inspector-status" title={copy.sections.status} level={3}>
            <SafeHStack as="div" wrap="wrap" gap={2} className={styles.statusFacts} aria-label={copy.sections.status}>
              <Badge variant={statusBadgeVariant(task.status)} label={copy.status[task.status]} />
              <Badge variant={priorityBadgeVariant(task.priority)} label={`${copy.editPriority} P${task.priority}`} />
              {task.dependencyBlocked ? <Badge variant="warning" label={copy.dependencyBlocked} /> : null}
            </SafeHStack>
          </Section>
          <Section id="inspector-metadata" title={copy.sections.readiness} level={3}>
            <Facts facts={[
              [copy.facts.requiredSteps, `${task.completedRequiredStepCount} / ${task.requiredStepCount}`],
              [copy.facts.blockedParents, String(task.unfinishedParentCount)],
              [copy.facts.statusReason, valueOrDash(task.statusReason)],
              [copy.facts.readiness, copy.planState[task.executionPlanState]],
            ]} />
          </Section>
          <Section id="inspector-result" title={copy.sections.result} level={3}>
            <ResultDisclosure summary={task.resultSummary} result={task.result} copy={copy} />
          </Section>
          <Section id="inspector-description" title={copy.description} level={3}>
            <DescriptionDisclosure description={task.description} copy={copy} />
          </Section>
        </InspectorGroup>

        <InspectorDisclosureGroup id="inspector-properties" title={copy.sections.properties}>
          <Section id="inspector-claim" title={copy.sections.execution} level={3}>
            <Facts facts={[
              [copy.facts.assignee, valueOrDash(task.assignee)],
              [copy.facts.plan, copy.planState[task.executionPlanState]],
              [copy.facts.requiredSteps, `${task.completedRequiredStepCount} / ${task.requiredStepCount}`],
              [copy.facts.optionalSteps, String(task.optionalStepCount)],
              [copy.facts.createdAt, String(task.createdAt)],
              [copy.facts.updatedAt, String(task.updatedAt)],
              [copy.facts.claimOwner, valueOrDash(task.claimOwner)],
              [copy.facts.claimExpires, valueOrDash(task.claimExpiresAt)],
              [copy.facts.heartbeat, valueOrDash(task.lastHeartbeatAt)],
              [copy.facts.currentRun, valueOrDash(task.currentRunId)],
              [copy.facts.retry, `${task.retryCount} / ${valueOrDash(task.maxRetries)}`],
              [copy.facts.blockedParents, String(task.unfinishedParentCount)],
            ]} />
          </Section>
          <section className={styles.propertyBlock} id="inspector-raw-metadata" data-testid="inspector-raw-metadata" aria-labelledby="inspector-raw-metadata-heading">
            <h3 id="inspector-raw-metadata-heading">{copy.sections.rawMetadata}</h3>
            <MetadataDisclosure metadata={task.metadata} copy={copy} />
          </section>
          <Section id="inspector-runtime" title={copy.sections.runtime} level={3}>
            <Facts facts={[
              [copy.facts.actor, model.runtime.actor],
              [copy.facts.api, model.runtime.apiBaseUrl || "/"],
              [copy.facts.server, model.runtime.serverVersion],
              [copy.facts.protocol, model.runtime.protocolVersion],
              [copy.facts.build, model.runtime.webBuildId],
            ]} />
          </Section>
        </InspectorDisclosureGroup>

        {!hideReadOnlyRelations ? (
          <InspectorGroup id="inspector-relations" title={copy.sections.relations}>
            <Section id="inspector-steps" title={copy.sections.steps} level={3}>
              {model.steps.length === 0 ? <Empty>{copy.noSteps}</Empty> : (
                <ol className={styles.compactList}>
                  {model.steps.map((step) => (
                    <li key={step.id} className={styles.row}>
                      <div>
                        <strong>{step.title}</strong>
                        {step.body ? <p className={styles.muted}>{step.body}</p> : null}
                      </div>
                      <Badge variant={stepStatusBadgeVariant(step.status)} label={`${copy.stepStatus[step.status]}${step.required ? ` · ${copy.required}` : ""}`} />
                    </li>
                  ))}
                </ol>
              )}
            </Section>

            <Section id="inspector-dependencies" title={copy.sections.dependencies} level={3}>
              <DependencyList title={copy.parents} tasks={model.parents} onSelectTask={onSelectTask} copy={copy} />
              <details>
                <summary>{copy.children}</summary>
                <DependencyList title={copy.children} tasks={model.children} onSelectTask={onSelectTask} copy={copy} />
              </details>
            </Section>
          </InspectorGroup>
        ) : null}

        <InspectorGroup id="inspector-activity" title={copy.sections.activity}>
          {!hideReadOnlyRelations ? (
            <Section id="inspector-comments" title={copy.sections.comments} level={3}>
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
          ) : null}

          <Section id="inspector-runs" title={copy.sections.runs} level={3}>
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

      <Section id="inspector-events" title={copy.sections.events} level={3}>
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

      <Section id="inspector-neighborhood" title={copy.sections.neighborhood} level={3}>
        <details ref={neighborhoodDetailsRef} onToggle={loadNeighborhood}>
          <summary>{copy.sections.neighborhood}</summary>
          {!neighborhood && (neighborhoodStatus === "loading" || neighborhoodStatus === "error" || neighborhoodStatus === "offline") ? <><Empty>{lazySectionNotice(neighborhoodStatus, online, copy)}</Empty><button type="button" onClick={() => startNeighborhoodLoad(true)}>{copy.retry}</button></> : neighborhood ? <>{lazySectionNotice(neighborhoodStatus, online, copy) ? <p className={styles.muted} role={neighborhoodStatus === "error" ? "alert" : "status"}>{lazySectionNotice(neighborhoodStatus, online, copy)}</p> : null}{neighborhoodStatus === "error" || neighborhoodStatus === "offline" ? <button type="button" onClick={() => startNeighborhoodLoad(true)}>{copy.retry}</button> : null}<Neighborhood model={neighborhood} copy={copy} onSelectTask={onSelectTask} /></> : <>{neighborhoodStatus === "stale" ? <p className={styles.muted} role="status">{copy.refreshPending}</p> : null}<Empty>{copy.noNeighborhood}</Empty></>}
        </details>
      </Section>

        </InspectorGroup>
      </div>
      {actionDialog ? <TaskInspectorActionDialog dialog={actionDialog} locale={locale} copy={copy} pending={mutationTransitionPending} error={transitionError?.message ?? null} onRetry={retryTransition} retryBlocksSubmit={transitionRetryMatches} onDescriptionChange={(description) => setActionDialog((current) => current?.kind === "description" ? { ...current, description } : current)} onReasonChange={(reason) => setActionDialog((current) => current?.kind === "reason" ? { ...current, reason } : current)} onConfirmationChange={(confirmed) => setActionDialog((current) => current?.kind === "reason" ? { ...current, confirmed } : current)} onCancel={closeActionDialog} onSubmit={submitActionDialog} /> : null}
    </aside>
  )
}
