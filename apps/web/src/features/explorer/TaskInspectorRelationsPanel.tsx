import { useRef, useState, type FormEvent } from "react"

import {
  inspectorMutationKey,
  type TaskInspectorMutationError,
  type TaskInspectorMutationHandlers,
  type TaskInspectorMutationSnapshot,
} from "./task-inspector-mutation-state"
import { buildCommentInput, buildPlanInput, buildStepSubmission, commentPageState, formatCommentDateTime, resolveTaskSelector, type CommentSortOrder, type TaskSelectorResolver } from "./TaskInspectorRelationsPanel.logic"
import styles from "./TaskInspectorRelationsPanel.module.css"

export type TaskInspectorRelationTaskStatus =
  | "triage"
  | "todo"
  | "scheduled"
  | "ready"
  | "running"
  | "blocked"
  | "review"
  | "done"
  | "archived"

export type TaskInspectorCommentKind = "note" | "decision" | "signal"

export interface TaskInspectorCommentView {
  readonly id: string
  readonly author: string
  readonly kind: TaskInspectorCommentKind
  readonly body: string
  readonly createdAt: number
  /** Structured metadata is rendered as escaped JSON; it is never interpreted as HTML. */
  readonly metadata?: Readonly<Record<string, unknown>>
}

export interface TaskInspectorRelationTaskView {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: TaskInspectorRelationTaskStatus
}

export interface TaskInspectorDependenciesView {
  readonly parents: readonly TaskInspectorRelationTaskView[]
  readonly children: readonly TaskInspectorRelationTaskView[]
}

export type TaskInspectorLinkedTaskView = TaskInspectorRelationTaskView

export interface TaskInspectorStepView {
  readonly id: string
  readonly title: string
  readonly body: string | null
  readonly required: boolean
  readonly status: "todo" | "done" | "skipped"
  readonly linkedTask?: TaskInspectorLinkedTaskView | null
}

export interface TaskInspectorExecutionPlanView {
  readonly state: "unplanned" | "planned" | "not_required"
  readonly reason?: string | null
}

export interface TaskInspectorStepsView {
  readonly steps: readonly TaskInspectorStepView[]
  readonly executionPlan?: TaskInspectorExecutionPlanView
}

export type TaskInspectorRelationsStepsInput = TaskInspectorStepsView | readonly TaskInspectorStepView[]

export interface TaskInspectorRelationsPanelProps {
  readonly taskId: string
  readonly comments: readonly TaskInspectorCommentView[]
  readonly dependencies: TaskInspectorDependenciesView
  readonly steps: TaskInspectorRelationsStepsInput
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly onSelectTask: (taskId: string) => void
  /** Synchronous same-board ref/id resolution seam owned by the parent read model. */
  readonly resolveTaskSelector: TaskSelectorResolver
  readonly locale?: "zh" | "en"
  readonly commentPageSize?: number
}

type RelationsCopy = {
  readonly comments: string
  readonly dependencies: string
  readonly steps: string
  readonly newest: string
  readonly oldest: string
  readonly commentSortLabel: string
  readonly commentCount: (count: number) => string
  readonly commentKind: string
  readonly commentBody: string
  readonly commentPlaceholder: string
  readonly requiredCommentBody: string
  readonly addComment: string
  readonly addingComment: string
  readonly parent: string
  readonly child: string
  readonly noComments: string
  readonly noDependencies: string
  readonly noSteps: string
  readonly dependencyInput: string
  readonly dependencyPlaceholder: string
  readonly requiredDependency: string
  readonly addDependency: string
  readonly addingDependency: string
  readonly removeParent: (title: string) => string
  readonly removingDependency: string
  readonly required: string
  readonly optional: string
  readonly todo: string
  readonly done: string
  readonly skipped: string
  readonly stepTitle: string
  readonly stepBody: string
  readonly stepBodyPlaceholder: string
  readonly requiredStepTitle: string
  readonly stepLink: string
  readonly stepLinkPlaceholder: string
  readonly createStep: string
  readonly createAndLinkStep: string
  readonly creatingStep: string
  readonly linkedTask: string
  readonly plan: string
  readonly planState: Readonly<Record<"unplanned" | "planned" | "not_required", string>>
  readonly planReason: string
  readonly planReasonPlaceholder: string
  readonly requiredPlanReason: string
  readonly markPlanNotRequired: string
  readonly markingPlanNotRequired: string
  readonly metadata: string
  readonly unresolvedDependency: string
  readonly unresolvedLinkedTask: string
  readonly page: (current: number, total: number) => string
  readonly previous: string
  readonly next: string
  readonly status: Readonly<Record<TaskInspectorRelationTaskStatus, string>>
  readonly commentKindLabel: Readonly<Record<TaskInspectorCommentKind, string>>
}

const copy: Record<"zh" | "en", RelationsCopy> = {
  zh: {
    comments: "评论",
    dependencies: "依赖",
    steps: "步骤",
    newest: "最新优先",
    oldest: "最早优先",
    commentSortLabel: "评论排序",
    commentCount: (count) => `${count} 条评论`,
    commentKind: "类型",
    commentBody: "评论内容",
    commentPlaceholder: "记录交接、决定或观察…",
    requiredCommentBody: "请输入评论内容。",
    addComment: "添加评论",
    addingComment: "正在添加评论…",
    parent: "父任务",
    child: "子任务",
    noComments: "暂无评论。",
    noDependencies: "暂无依赖。",
    noSteps: "暂无步骤。",
    dependencyInput: "父任务 ref 或 id",
    dependencyPlaceholder: "例如 default#12 或 t_parent…",
    requiredDependency: "请输入父任务 ref 或 id。",
    addDependency: "添加父依赖",
    addingDependency: "正在添加依赖…",
    removeParent: (title) => `移除父依赖：${title}`,
    removingDependency: "正在移除…",
    required: "必需",
    optional: "可选",
    todo: "待办",
    done: "已完成",
    skipped: "已跳过",
    stepTitle: "步骤标题",
    stepBody: "步骤说明",
    stepBodyPlaceholder: "可选说明…",
    requiredStepTitle: "请输入步骤标题。",
    stepLink: "关联任务 ref 或 id",
    stepLinkPlaceholder: "可选，例如 default#13…",
    createStep: "创建步骤",
    createAndLinkStep: "创建并关联步骤",
    creatingStep: "正在创建步骤…",
    linkedTask: "关联任务",
    plan: "执行计划",
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
    planReason: "无需计划原因",
    planReasonPlaceholder: "说明为何无需执行计划…",
    requiredPlanReason: "请输入无需计划原因。",
    markPlanNotRequired: "标记为无需计划",
    markingPlanNotRequired: "正在更新计划…",
    metadata: "元数据",
    unresolvedDependency: "无法解析该父任务 ref 或 id。",
    unresolvedLinkedTask: "无法解析该关联任务 ref 或 id。",
    page: (current, total) => `第 ${current} / ${total} 页`,
    previous: "上一页",
    next: "下一页",
    status: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    commentKindLabel: { note: "备注", decision: "决策", signal: "信号" },
  },
  en: {
    comments: "Comments",
    dependencies: "Dependencies",
    steps: "Steps",
    newest: "Newest first",
    oldest: "Oldest first",
    commentSortLabel: "Comment sort order",
    commentCount: (count) => `${count} comments`,
    commentKind: "Kind",
    commentBody: "Comment body",
    commentPlaceholder: "Record a handoff, decision, or observation…",
    requiredCommentBody: "Enter a comment body.",
    addComment: "Add comment",
    addingComment: "Adding comment…",
    parent: "Parents",
    child: "Children",
    noComments: "No comments.",
    noDependencies: "No dependencies.",
    noSteps: "No steps.",
    dependencyInput: "Parent task ref or id",
    dependencyPlaceholder: "For example default#12 or t_parent…",
    requiredDependency: "Enter a parent task ref or id.",
    addDependency: "Add parent dependency",
    addingDependency: "Adding dependency…",
    removeParent: (title) => `Remove parent dependency: ${title}`,
    removingDependency: "Removing…",
    required: "Required",
    optional: "Optional",
    todo: "To do",
    done: "Done",
    skipped: "Skipped",
    stepTitle: "Step title",
    stepBody: "Step description",
    stepBodyPlaceholder: "Optional description…",
    requiredStepTitle: "Enter a step title.",
    stepLink: "Linked task ref or id",
    stepLinkPlaceholder: "Optional, for example default#13…",
    createStep: "Create step",
    createAndLinkStep: "Create & link step",
    creatingStep: "Creating step…",
    linkedTask: "Linked task",
    plan: "Execution plan",
    planState: { unplanned: "Unplanned", planned: "Planned", not_required: "Not required" },
    planReason: "Why no plan is needed",
    planReasonPlaceholder: "Explain why an execution plan is not needed…",
    requiredPlanReason: "Enter a reason for not requiring a plan.",
    markPlanNotRequired: "Mark plan not required",
    markingPlanNotRequired: "Updating plan…",
    metadata: "Metadata",
    unresolvedDependency: "The parent task ref or id could not be resolved.",
    unresolvedLinkedTask: "The linked task ref or id could not be resolved.",
    page: (current, total) => `Page ${current} of ${total}`,
    previous: "Previous",
    next: "Next",
    status: { triage: "Triage", todo: "To do", scheduled: "Scheduled", ready: "Ready", running: "Running", blocked: "Blocked", review: "Review", done: "Done", archived: "Archived" },
    commentKindLabel: { note: "Note", decision: "Decision", signal: "Signal" },
  },
}

function asStepsView(value: TaskInspectorRelationsStepsInput): TaskInspectorStepsView {
  if ("steps" in value) return value
  return { steps: value }
}

function safeJson(value: Readonly<Record<string, unknown>> | undefined): string {
  if (value === undefined) return "{}"
  try {
    return JSON.stringify(value) ?? "{}"
  } catch {
    return "[unserializable metadata]"
  }
}

function errorFor(
  snapshot: TaskInspectorMutationSnapshot,
  operation: TaskInspectorMutationError["operation"],
  taskId: string,
): TaskInspectorMutationError | null {
  const direct = snapshot.errors.get(inspectorMutationKey(operation, taskId))
  if (direct) return direct
  for (const error of snapshot.errors.values()) {
    if (error.operation === operation && error.taskId === taskId) return error
  }
  return null
}

function pendingFor(snapshot: TaskInspectorMutationSnapshot, operation: string, taskId: string): boolean {
  return snapshot.pending.has(inspectorMutationKey(operation as Parameters<typeof inspectorMutationKey>[0], taskId))
}

function Feedback({ error, pendingLabel }: { readonly error: TaskInspectorMutationError | null; readonly pendingLabel?: string }) {
  return (
    <>
      {pendingLabel ? <p className={styles.pending} role="status" aria-live="polite">{pendingLabel}</p> : null}
      {error ? (
        <p className={styles.error} role="alert" aria-live="polite">
          {error.message}{error.code ? ` [${error.code}]` : ""}
        </p>
      ) : null}
    </>
  )
}

function StatusLabel({ status, currentCopy }: { readonly status: TaskInspectorStepView["status"]; readonly currentCopy: RelationsCopy }) {
  return currentCopy[status]
}

function RelationTaskButton({ task, onSelectTask, localeCopy }: { readonly task: TaskInspectorRelationTaskView; readonly onSelectTask: (taskId: string) => void; readonly localeCopy: RelationsCopy }) {
  return (
    <button
      type="button"
      className={styles.taskLink}
      onClick={() => onSelectTask(task.id)}
      aria-label={`${task.ref} ${task.title}`}
    >
      <span className={styles.ref} translate="no">{task.ref}</span>
      <span className={styles.taskTitle}>{task.title}</span>
      <span className={styles.muted}>{localeCopy.status[task.status]}</span>
    </button>
  )
}

function CommentsPanel({
  taskId,
  comments,
  handlers,
  snapshot,
  localeCopy,
  locale,
  pageSize,
}: {
  readonly taskId: string
  readonly comments: readonly TaskInspectorCommentView[]
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly localeCopy: RelationsCopy
  readonly locale: "zh" | "en"
  readonly pageSize: number
}) {
  const [sortOrder, setSortOrder] = useState<CommentSortOrder>("newest")
  const [page, setPage] = useState(0)
  const [kind, setKind] = useState<TaskInspectorCommentKind>("note")
  const [body, setBody] = useState("")
  const [bodyError, setBodyError] = useState<string | null>(null)
  const bodyRef = useRef<HTMLTextAreaElement | null>(null)
  const pageState = commentPageState(comments, page, pageSize, sortOrder)
  const pending = pendingFor(snapshot, "addComment", taskId)
  const error = errorFor(snapshot, "addComment", taskId)

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const input = buildCommentInput(kind, body)
    if (!input.body) {
      setBodyError(localeCopy.requiredCommentBody)
      bodyRef.current?.focus()
      return
    }
    try {
      await handlers.addComment(input)
      setBody("")
      setKind("note")
      setBodyError(null)
    } catch {
      // The shared controller owns the recoverable error snapshot. Keep draft text for retry.
    }
  }

  return (
    <section className={styles.section} data-testid="task-inspector-comments">
      <div className={styles.sectionHeading}>
        <h2>{localeCopy.comments}</h2>
        <span className={styles.muted}>{localeCopy.commentCount(comments.length)}</span>
      </div>
      {comments.length === 0 ? <p className={styles.empty} role="status">{localeCopy.noComments}</p> : (
        <>
          <div className={styles.toolbar}>
            <label className={styles.inlineField} htmlFor="task-inspector-comments-sort">
              <span className={styles.visuallyHidden}>{localeCopy.commentSortLabel}</span>
              <select
                id="task-inspector-comments-sort"
                data-testid="task-inspector-comments-sort"
                name="comments-sort"
                value={sortOrder}
                aria-label={localeCopy.commentSortLabel}
                onChange={(event) => {
                  setSortOrder(event.currentTarget.value as CommentSortOrder)
                  setPage(0)
                }}
              >
                <option value="newest">{localeCopy.newest}</option>
                <option value="oldest">{localeCopy.oldest}</option>
              </select>
            </label>
          </div>
          <ul className={styles.list}>
            {pageState.comments.map((comment) => (
              <li className={styles.card} key={comment.id}>
                <div className={styles.cardMeta}>
                  <span><strong>{comment.author}</strong><span className={styles.muted}> · {localeCopy.commentKindLabel[comment.kind]}</span></span>
                  {(() => {
                    const renderedTime = formatCommentDateTime(comment.createdAt, locale)
                    return <time dateTime={renderedTime.iso || undefined}>{renderedTime.label}</time>
                  })()}
                </div>
                <p className={styles.body}>{comment.body}</p>
                <details className={styles.metadata}>
                  <summary>{localeCopy.metadata}</summary>
                  <pre>{safeJson(comment.metadata)}</pre>
                </details>
              </li>
            ))}
          </ul>
          {pageState.pageCount > 1 ? (
            <div className={styles.pager}>
              <button type="button" onClick={() => setPage((value) => Math.max(0, value - 1))} disabled={!pageState.hasPreviousPage}>{localeCopy.previous}</button>
              <span aria-live="polite">{localeCopy.page(pageState.page + 1, pageState.pageCount)}</span>
              <button data-testid="task-inspector-comments-next" type="button" onClick={() => setPage((value) => Math.min(pageState.pageCount - 1, value + 1))} disabled={!pageState.hasNextPage}>{localeCopy.next}</button>
            </div>
          ) : null}
        </>
      )}
      <form className={styles.form} onSubmit={(event) => void submit(event)} aria-busy={pending || undefined}>
        <label>
          <span>{localeCopy.commentKind}</span>
          <select name="comment-kind" value={kind} onChange={(event) => setKind(event.currentTarget.value as TaskInspectorCommentKind)}>
            <option value="note">{localeCopy.commentKindLabel.note}</option>
            <option value="decision">{localeCopy.commentKindLabel.decision}</option>
            <option value="signal">{localeCopy.commentKindLabel.signal}</option>
          </select>
        </label>
        <label>
          <span>{localeCopy.commentBody}</span>
          <textarea ref={bodyRef} name="comment-body" autoComplete="off" required aria-required="true" aria-invalid={bodyError ? "true" : "false"} aria-describedby={bodyError ? "task-inspector-comment-body-error" : undefined} value={body} onChange={(event) => { setBody(event.currentTarget.value); setBodyError(null) }} onInvalid={() => setBodyError(localeCopy.requiredCommentBody)} placeholder={localeCopy.commentPlaceholder} />
        </label>
        {bodyError ? <p id="task-inspector-comment-body-error" className={styles.error} role="alert" aria-live="polite">{bodyError}</p> : null}
        <button type="submit" disabled={pending}>{pending ? localeCopy.addingComment : localeCopy.addComment}</button>
        <Feedback error={error} pendingLabel={pending ? localeCopy.addingComment : undefined} />
      </form>
    </section>
  )
}

function DependenciesPanel({
  taskId,
  dependencies,
  handlers,
  snapshot,
  onSelectTask,
  resolveSelector,
  localeCopy,
}: {
  readonly taskId: string
  readonly dependencies: TaskInspectorDependenciesView
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly onSelectTask: (taskId: string) => void
  readonly resolveSelector: TaskSelectorResolver
  readonly localeCopy: RelationsCopy
}) {
  const [input, setInput] = useState("")
  const [resolutionError, setResolutionError] = useState<string | null>(null)
  const inputRef = useRef<HTMLInputElement | null>(null)
  const addPending = pendingFor(snapshot, "addDependency", taskId)
  const addError = errorFor(snapshot, "addDependency", taskId)
  const removePending = pendingFor(snapshot, "removeDependency", taskId)
  const removeError = errorFor(snapshot, "removeDependency", taskId)

  const add = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const value = input.trim()
    if (!value) {
      setResolutionError(localeCopy.requiredDependency)
      inputRef.current?.focus()
      return
    }
    const parentTaskId = resolveTaskSelector(value, resolveSelector)
    if (!parentTaskId) {
      setResolutionError(localeCopy.unresolvedDependency)
      return
    }
    try {
      await handlers.addDependency(parentTaskId)
      setInput("")
      setResolutionError(null)
    } catch {
      // Keep the ref/id visible so the controller can retry the same intent.
    }
  }

  return (
    <section className={styles.section} data-testid="task-inspector-dependencies">
      <h2>{localeCopy.dependencies}</h2>
      <div className={styles.dependencyGroup}>
        <h3>{localeCopy.parent}</h3>
        {dependencies.parents.length === 0 ? <p className={styles.empty} role="status">{localeCopy.noDependencies}</p> : (
          <ul className={styles.list}>
            {dependencies.parents.map((task) => (
              <li className={styles.relationRow} key={task.id}>
                <RelationTaskButton task={task} onSelectTask={onSelectTask} localeCopy={localeCopy} />
                <button type="button" className={styles.removeButton} disabled={removePending} onClick={() => void handlers.removeDependency(task.id)} aria-label={localeCopy.removeParent(task.title)}>
                  {removePending ? localeCopy.removingDependency : "×"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className={styles.dependencyGroup}>
        <h3>{localeCopy.child}</h3>
        {dependencies.children.length === 0 ? <p className={styles.empty} role="status">{localeCopy.noDependencies}</p> : (
          <ul className={styles.list}>
            {dependencies.children.map((task) => <li className={styles.relationRow} key={task.id}><RelationTaskButton task={task} onSelectTask={onSelectTask} localeCopy={localeCopy} /></li>)}
          </ul>
        )}
      </div>
      <form className={styles.form} onSubmit={(event) => void add(event)} aria-busy={addPending || undefined}>
        <label>
          <span>{localeCopy.dependencyInput}</span>
          <input ref={inputRef} name="dependency-parent" autoComplete="off" required aria-required="true" aria-invalid={resolutionError ? "true" : "false"} aria-describedby={resolutionError ? "task-inspector-dependency-error" : undefined} value={input} onChange={(event) => { setInput(event.currentTarget.value); setResolutionError(null) }} onInvalid={() => setResolutionError(localeCopy.requiredDependency)} placeholder={localeCopy.dependencyPlaceholder} />
        </label>
        <button type="submit" disabled={addPending}>{addPending ? localeCopy.addingDependency : localeCopy.addDependency}</button>
        <Feedback error={addError} pendingLabel={addPending ? localeCopy.addingDependency : undefined} />
        <Feedback error={removeError} pendingLabel={removePending ? localeCopy.removingDependency : undefined} />
        {resolutionError ? <p id="task-inspector-dependency-error" className={styles.error} role="alert" aria-live="polite">{resolutionError}</p> : null}
      </form>
    </section>
  )
}

function StepsPanel({
  taskId,
  stepsInput,
  handlers,
  snapshot,
  onSelectTask,
  resolveSelector,
  localeCopy,
}: {
  readonly taskId: string
  readonly stepsInput: TaskInspectorRelationsStepsInput
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly onSelectTask: (taskId: string) => void
  readonly resolveSelector: TaskSelectorResolver
  readonly localeCopy: RelationsCopy
}) {
  const [title, setTitle] = useState("")
  const [body, setBody] = useState("")
  const [required, setRequired] = useState(true)
  const [linkedTaskRef, setLinkedTaskRef] = useState("")
  const [linkedTaskResolutionError, setLinkedTaskResolutionError] = useState<string | null>(null)
  const [titleError, setTitleError] = useState<string | null>(null)
  const titleRef = useRef<HTMLInputElement | null>(null)
  const [planReason, setPlanReason] = useState("")
  const [planReasonError, setPlanReasonError] = useState<string | null>(null)
  const planReasonRef = useRef<HTMLInputElement | null>(null)
  const stepsView = asStepsView(stepsInput)
  const createPending = pendingFor(snapshot, "createStep", taskId)
  const createError = errorFor(snapshot, "createStep", taskId)
  const linkPending = pendingFor(snapshot, "linkStep", taskId)
  const linkError = errorFor(snapshot, "linkStep", taskId)
  const planPending = pendingFor(snapshot, "markPlanNotRequired", taskId)
  const planError = errorFor(snapshot, "markPlanNotRequired", taskId)
  const stepPending = createPending || linkPending
  const plan = stepsView.executionPlan

  const submitStep = async (link: boolean) => {
    if (!title.trim()) {
      setTitleError(localeCopy.requiredStepTitle)
      titleRef.current?.focus()
      return
    }
    const linkedTaskId = link && linkedTaskRef.trim() ? resolveTaskSelector(linkedTaskRef, resolveSelector) : null
    if (link && !linkedTaskId) {
      setLinkedTaskResolutionError(localeCopy.unresolvedLinkedTask)
      return
    }
    const submission = buildStepSubmission(title, body, required, linkedTaskId ?? undefined, link ? "link" : "create")
    if (!submission) return
    try {
      if (submission.operation === "linkStep") await handlers.linkStep(submission.input)
      else await handlers.createStep(submission.input)
      setTitle("")
      setBody("")
      setRequired(true)
      setLinkedTaskRef("")
      setLinkedTaskResolutionError(null)
      setTitleError(null)
    } catch {
      // Keep the draft available for a retry.
    }
  }

  const submitPlan = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const input = buildPlanInput(planReason)
    if (!input.reason) {
      setPlanReasonError(localeCopy.requiredPlanReason)
      planReasonRef.current?.focus()
      return
    }
    try {
      await handlers.markPlanNotRequired(input)
      setPlanReason("")
      setPlanReasonError(null)
    } catch {
      // Keep the reason available for a retry.
    }
  }

  return (
    <section className={styles.section} data-testid="task-inspector-steps">
      <div className={styles.sectionHeading}>
        <h2>{localeCopy.steps}</h2>
        {plan ? <span className={styles.badge}>{localeCopy.plan}: {localeCopy.planState[plan.state]}</span> : null}
      </div>
      {stepsView.steps.length === 0 ? <p className={styles.empty} role="status">{localeCopy.noSteps}</p> : (
        <ul className={styles.list}>
          {stepsView.steps.map((step) => (
            <li className={styles.card} key={step.id}>
              <div className={styles.cardMeta}>
                <strong>{step.title}</strong>
                <span className={styles.badge}>{step.required ? localeCopy.required : localeCopy.optional} · <StatusLabel status={step.status} currentCopy={localeCopy} /></span>
              </div>
              {step.body ? <p className={styles.body}>{step.body}</p> : null}
              {step.linkedTask ? (
                <div className={styles.linkedTask}>
                  <span className={styles.muted}>{localeCopy.linkedTask}</span>
                  <RelationTaskButton task={step.linkedTask} onSelectTask={onSelectTask} localeCopy={localeCopy} />
                </div>
              ) : null}
            </li>
          ))}
        </ul>
      )}
      <form className={styles.form} onSubmit={(event) => { event.preventDefault(); void submitStep(false) }} aria-busy={stepPending || undefined}>
        <label>
          <span>{localeCopy.stepTitle}</span>
          <input ref={titleRef} data-testid="task-inspector-step-title" name="step-title" autoComplete="off" required aria-required="true" aria-invalid={titleError ? "true" : "false"} aria-describedby={titleError ? "task-inspector-step-title-error" : undefined} value={title} onChange={(event) => { setTitle(event.currentTarget.value); setTitleError(null) }} onInvalid={() => setTitleError(localeCopy.requiredStepTitle)} />
        </label>
        {titleError ? <p id="task-inspector-step-title-error" className={styles.error} role="alert" aria-live="polite">{titleError}</p> : null}
        <label>
          <span>{localeCopy.stepBody}</span>
          <textarea name="step-body" autoComplete="off" value={body} onChange={(event) => setBody(event.currentTarget.value)} placeholder={localeCopy.stepBodyPlaceholder} />
        </label>
        <label className={styles.checkbox}>
          <input type="checkbox" name="step-required" checked={required} onChange={(event) => setRequired(event.currentTarget.checked)} />
          <span>{localeCopy.required}</span>
        </label>
        <label>
          <span>{localeCopy.stepLink}</span>
          <input name="step-linked-task" autoComplete="off" aria-invalid={linkedTaskResolutionError ? "true" : "false"} aria-describedby={linkedTaskResolutionError ? "task-inspector-step-link-error" : undefined} value={linkedTaskRef} onChange={(event) => { setLinkedTaskRef(event.currentTarget.value); setLinkedTaskResolutionError(null) }} placeholder={localeCopy.stepLinkPlaceholder} />
        </label>
        <div className={styles.buttonRow}>
          <button data-testid="task-inspector-create-step" type="button" disabled={stepPending} onClick={() => void submitStep(false)}>{stepPending ? localeCopy.creatingStep : localeCopy.createStep}</button>
          <button data-testid="task-inspector-link-step" type="button" disabled={stepPending} onClick={() => void submitStep(true)}>{stepPending ? localeCopy.creatingStep : localeCopy.createAndLinkStep}</button>
        </div>
        <Feedback error={createError} pendingLabel={createPending ? localeCopy.creatingStep : undefined} />
        <Feedback error={linkError} pendingLabel={linkPending ? localeCopy.creatingStep : undefined} />
        {linkedTaskResolutionError ? <p id="task-inspector-step-link-error" className={styles.error} role="alert" aria-live="polite">{linkedTaskResolutionError}</p> : null}
      </form>
      <form className={styles.form} onSubmit={(event) => void submitPlan(event)} aria-busy={planPending || undefined}>
        <label>
          <span>{localeCopy.planReason}</span>
          <input ref={planReasonRef} name="plan-not-required-reason" autoComplete="off" required aria-required="true" aria-invalid={planReasonError ? "true" : "false"} aria-describedby={planReasonError ? "task-inspector-plan-reason-error" : undefined} value={planReason} onChange={(event) => { setPlanReason(event.currentTarget.value); setPlanReasonError(null) }} onInvalid={() => setPlanReasonError(localeCopy.requiredPlanReason)} placeholder={localeCopy.planReasonPlaceholder} />
        </label>
        {planReasonError ? <p id="task-inspector-plan-reason-error" className={styles.error} role="alert" aria-live="polite">{planReasonError}</p> : null}
        <button data-testid="task-inspector-mark-plan-not-required" type="submit" disabled={planPending || plan?.state === "not_required"}>{planPending ? localeCopy.markingPlanNotRequired : localeCopy.markPlanNotRequired}</button>
        <Feedback error={planError} pendingLabel={planPending ? localeCopy.markingPlanNotRequired : undefined} />
      </form>
    </section>
  )
}

export function TaskInspectorRelationsPanel({
  taskId,
  comments,
  dependencies,
  steps,
  handlers,
  snapshot,
  onSelectTask,
  resolveTaskSelector: resolveSelector,
  locale = "zh",
  commentPageSize = 10,
}: TaskInspectorRelationsPanelProps) {
  const localeCopy = copy[locale]
  const safePageSize = Number.isSafeInteger(commentPageSize) && commentPageSize > 0 ? commentPageSize : 10
  return (
    <div className={styles.relations} data-testid="task-inspector-relations">
      <CommentsPanel taskId={taskId} comments={comments} handlers={handlers} snapshot={snapshot} localeCopy={localeCopy} locale={locale} pageSize={safePageSize} />
      <DependenciesPanel taskId={taskId} dependencies={dependencies} handlers={handlers} snapshot={snapshot} onSelectTask={onSelectTask} resolveSelector={resolveSelector} localeCopy={localeCopy} />
      <StepsPanel taskId={taskId} stepsInput={steps} handlers={handlers} snapshot={snapshot} onSelectTask={onSelectTask} resolveSelector={resolveSelector} localeCopy={localeCopy} />
    </div>
  )
}
