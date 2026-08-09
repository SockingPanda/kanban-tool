import { useMemo, useState, type FormEvent } from "react"

import {
  inspectorMutationKey,
  type TaskInspectorMutationError,
  type TaskInspectorMutationHandlers,
  type TaskInspectorMutationSnapshot,
} from "./task-inspector-mutation-state"
import { buildCommentInput, buildPlanInput, buildStepInput } from "./TaskInspectorRelationsPanel.logic"
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
  readonly locale?: "zh" | "en"
  readonly commentPageSize?: number
}

type RelationsCopy = {
  readonly comments: string
  readonly dependencies: string
  readonly steps: string
  readonly newest: string
  readonly oldest: string
  readonly commentCount: (count: number) => string
  readonly commentAuthor: string
  readonly commentKind: string
  readonly commentBody: string
  readonly commentPlaceholder: string
  readonly addComment: string
  readonly addingComment: string
  readonly parent: string
  readonly child: string
  readonly noComments: string
  readonly noDependencies: string
  readonly noSteps: string
  readonly dependencyInput: string
  readonly dependencyPlaceholder: string
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
  readonly markPlanNotRequired: string
  readonly markingPlanNotRequired: string
  readonly metadata: string
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
    commentCount: (count) => `${count} 条评论`,
    commentAuthor: "作者",
    commentKind: "类型",
    commentBody: "评论内容",
    commentPlaceholder: "记录交接、决定或观察…",
    addComment: "添加评论",
    addingComment: "正在添加评论…",
    parent: "父任务",
    child: "子任务",
    noComments: "暂无评论。",
    noDependencies: "暂无依赖。",
    noSteps: "暂无步骤。",
    dependencyInput: "父任务 ref 或 id",
    dependencyPlaceholder: "例如 default#12 或 t_parent…",
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
    markPlanNotRequired: "标记为无需计划",
    markingPlanNotRequired: "正在更新计划…",
    metadata: "元数据",
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
    commentCount: (count) => `${count} comments`,
    commentAuthor: "Author",
    commentKind: "Kind",
    commentBody: "Comment body",
    commentPlaceholder: "Record a handoff, decision, or observation…",
    addComment: "Add comment",
    addingComment: "Adding comment…",
    parent: "Parents",
    child: "Children",
    noComments: "No comments.",
    noDependencies: "No dependencies.",
    noSteps: "No steps.",
    dependencyInput: "Parent task ref or id",
    dependencyPlaceholder: "For example default#12 or t_parent…",
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
    markPlanNotRequired: "Mark plan not required",
    markingPlanNotRequired: "Updating plan…",
    metadata: "Metadata",
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
  pageSize,
}: {
  readonly taskId: string
  readonly comments: readonly TaskInspectorCommentView[]
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly localeCopy: RelationsCopy
  readonly pageSize: number
}) {
  const [sortOrder, setSortOrder] = useState<"newest" | "oldest">("newest")
  const [page, setPage] = useState(0)
  const [author, setAuthor] = useState("")
  const [kind, setKind] = useState<TaskInspectorCommentKind>("note")
  const [body, setBody] = useState("")
  const sortedComments = useMemo(() => [...comments].sort((left, right) => {
    const createdDiff = left.createdAt - right.createdAt
    const idDiff = left.id.localeCompare(right.id)
    const diff = createdDiff || idDiff
    return sortOrder === "newest" ? -diff : diff
  }), [comments, sortOrder])
  const pageCount = Math.max(1, Math.ceil(sortedComments.length / pageSize))
  const currentPage = Math.min(Math.max(page, 0), pageCount - 1)
  const visibleComments = sortedComments.slice(currentPage * pageSize, currentPage * pageSize + pageSize)
  const pending = pendingFor(snapshot, "addComment", taskId)
  const error = errorFor(snapshot, "addComment", taskId)

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const input = buildCommentInput(author, kind, body)
    if (!input.author || !input.body) return
    try {
      await handlers.addComment(input)
      setAuthor("")
      setBody("")
      setKind("note")
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
              <span className={styles.visuallyHidden}>{localeCopy.comments}</span>
              <select
                id="task-inspector-comments-sort"
                data-testid="task-inspector-comments-sort"
                name="comments-sort"
                value={sortOrder}
                onChange={(event) => {
                  setSortOrder(event.currentTarget.value as "newest" | "oldest")
                  setPage(0)
                }}
              >
                <option value="newest">{localeCopy.newest}</option>
                <option value="oldest">{localeCopy.oldest}</option>
              </select>
            </label>
          </div>
          <ul className={styles.list}>
            {visibleComments.map((comment) => (
              <li className={styles.card} key={comment.id}>
                <div className={styles.cardMeta}>
                  <span><strong>{comment.author}</strong><span className={styles.muted}> · {localeCopy.commentKindLabel[comment.kind]}</span></span>
                  <time dateTime={String(comment.createdAt)}>{comment.createdAt}</time>
                </div>
                <p className={styles.body}>{comment.body}</p>
                <details className={styles.metadata}>
                  <summary>{localeCopy.metadata}</summary>
                  <pre>{safeJson(comment.metadata)}</pre>
                </details>
              </li>
            ))}
          </ul>
          {pageCount > 1 ? (
            <div className={styles.pager}>
              <button type="button" onClick={() => setPage((value) => Math.max(0, value - 1))} disabled={currentPage === 0}>{localeCopy.previous}</button>
              <span aria-live="polite">{localeCopy.page(currentPage + 1, pageCount)}</span>
              <button data-testid="task-inspector-comments-next" type="button" onClick={() => setPage((value) => Math.min(pageCount - 1, value + 1))} disabled={currentPage >= pageCount - 1}>{localeCopy.next}</button>
            </div>
          ) : null}
        </>
      )}
      <form className={styles.form} onSubmit={(event) => void submit(event)} aria-busy={pending || undefined}>
        <div className={styles.formGrid}>
          <label>
            <span>{localeCopy.commentAuthor}</span>
            <input data-testid="task-inspector-comment-author" name="comment-author" autoComplete="off" value={author} onChange={(event) => setAuthor(event.currentTarget.value)} />
          </label>
          <label>
            <span>{localeCopy.commentKind}</span>
            <select name="comment-kind" value={kind} onChange={(event) => setKind(event.currentTarget.value as TaskInspectorCommentKind)}>
              <option value="note">note</option>
              <option value="decision">decision</option>
              <option value="signal">signal</option>
            </select>
          </label>
        </div>
        <label>
          <span>{localeCopy.commentBody}</span>
          <textarea name="comment-body" autoComplete="off" value={body} onChange={(event) => setBody(event.currentTarget.value)} placeholder={localeCopy.commentPlaceholder} />
        </label>
        <button type="submit" disabled={pending || !author.trim() || !body.trim()}>{pending ? localeCopy.addingComment : localeCopy.addComment}</button>
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
  localeCopy,
}: {
  readonly taskId: string
  readonly dependencies: TaskInspectorDependenciesView
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly onSelectTask: (taskId: string) => void
  readonly localeCopy: RelationsCopy
}) {
  const [input, setInput] = useState("")
  const addPending = pendingFor(snapshot, "addDependency", taskId)
  const addError = errorFor(snapshot, "addDependency", taskId)
  const removePending = pendingFor(snapshot, "removeDependency", taskId)
  const removeError = errorFor(snapshot, "removeDependency", taskId)

  const add = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const value = input.trim()
    if (!value) return
    try {
      await handlers.addDependency(value)
      setInput("")
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
          <input name="dependency-parent" autoComplete="off" value={input} onChange={(event) => setInput(event.currentTarget.value)} placeholder={localeCopy.dependencyPlaceholder} />
        </label>
        <button type="submit" disabled={addPending || !input.trim()}>{addPending ? localeCopy.addingDependency : localeCopy.addDependency}</button>
        <Feedback error={addError ?? removeError} pendingLabel={addPending ? localeCopy.addingDependency : removePending ? localeCopy.removingDependency : undefined} />
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
  localeCopy,
}: {
  readonly taskId: string
  readonly stepsInput: TaskInspectorRelationsStepsInput
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly onSelectTask: (taskId: string) => void
  readonly localeCopy: RelationsCopy
}) {
  const [title, setTitle] = useState("")
  const [body, setBody] = useState("")
  const [required, setRequired] = useState(true)
  const [linkedTaskRef, setLinkedTaskRef] = useState("")
  const [planReason, setPlanReason] = useState("")
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
    const input = buildStepInput(title, body, required, link ? linkedTaskRef : undefined)
    if (!input.title || (link && !input.linked_task_ref)) return
    try {
      if (link) await handlers.linkStep(input)
      else await handlers.createStep(input)
      setTitle("")
      setBody("")
      setRequired(true)
      setLinkedTaskRef("")
    } catch {
      // Keep the draft available for a retry.
    }
  }

  const submitPlan = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const input = buildPlanInput(planReason)
    if (!input.reason) return
    try {
      await handlers.markPlanNotRequired(input)
      setPlanReason("")
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
      <form className={styles.form} onSubmit={(event) => { event.preventDefault(); void submitStep(Boolean(linkedTaskRef.trim())) }} aria-busy={stepPending || undefined}>
        <label>
          <span>{localeCopy.stepTitle}</span>
          <input data-testid="task-inspector-step-title" name="step-title" autoComplete="off" value={title} onChange={(event) => setTitle(event.currentTarget.value)} />
        </label>
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
          <input name="step-linked-task" autoComplete="off" value={linkedTaskRef} onChange={(event) => setLinkedTaskRef(event.currentTarget.value)} placeholder={localeCopy.stepLinkPlaceholder} />
        </label>
        <div className={styles.buttonRow}>
          <button data-testid="task-inspector-create-step" type="button" disabled={stepPending || !title.trim()} onClick={() => void submitStep(false)}>{stepPending ? localeCopy.creatingStep : localeCopy.createStep}</button>
          <button data-testid="task-inspector-link-step" type="button" disabled={stepPending || !title.trim() || !linkedTaskRef.trim()} onClick={() => void submitStep(true)}>{stepPending ? localeCopy.creatingStep : localeCopy.createAndLinkStep}</button>
        </div>
        <Feedback error={createError ?? linkError} pendingLabel={stepPending ? localeCopy.creatingStep : undefined} />
      </form>
      <form className={styles.form} onSubmit={(event) => void submitPlan(event)} aria-busy={planPending || undefined}>
        <label>
          <span>{localeCopy.planReason}</span>
          <input name="plan-not-required-reason" autoComplete="off" value={planReason} onChange={(event) => setPlanReason(event.currentTarget.value)} placeholder={localeCopy.planReasonPlaceholder} />
        </label>
        <button data-testid="task-inspector-mark-plan-not-required" type="submit" disabled={planPending || !planReason.trim() || plan?.state === "not_required"}>{planPending ? localeCopy.markingPlanNotRequired : localeCopy.markPlanNotRequired}</button>
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
  locale = "zh",
  commentPageSize = 10,
}: TaskInspectorRelationsPanelProps) {
  const localeCopy = copy[locale]
  const safePageSize = Number.isSafeInteger(commentPageSize) && commentPageSize > 0 ? commentPageSize : 10
  return (
    <div className={styles.relations} data-testid="task-inspector-relations">
      <CommentsPanel taskId={taskId} comments={comments} handlers={handlers} snapshot={snapshot} localeCopy={localeCopy} pageSize={safePageSize} />
      <DependenciesPanel taskId={taskId} dependencies={dependencies} handlers={handlers} snapshot={snapshot} onSelectTask={onSelectTask} localeCopy={localeCopy} />
      <StepsPanel taskId={taskId} stepsInput={steps} handlers={handlers} snapshot={snapshot} onSelectTask={onSelectTask} localeCopy={localeCopy} />
    </div>
  )
}
