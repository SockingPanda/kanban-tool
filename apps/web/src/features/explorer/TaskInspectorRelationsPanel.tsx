import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react"
import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Collapsible } from "@astryxdesign/core/Collapsible"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { Text } from "@astryxdesign/core/Text"

import {
  inspectorMutationKey,
  type TaskInspectorMutationError,
  type InspectorMutationOutcome,
  type TaskInspectorMutationHandlers,
  type TaskInspectorMutationSnapshot,
} from "./task-inspector-mutation-state"
import { buildCommentInput, buildPlanInput, buildStepSubmission, commentDraftMatchesRetry, commentPageState, dependencyDraftMatchesRetry, formatCommentDateTime, planDraftMatchesRetry, resolveTaskSelector, scopeEpochMatches, shouldClearDraft, shouldClearRetryDraft, stepDraftMatchesRetry, type CommentSortOrder, type InspectorScopeEpoch, type TaskSelectorResolver } from "./TaskInspectorRelationsPanel.logic"
import {
  CheckboxInput,
  CodeBlock,
  SafeHStack,
  SafeVStack,
  Selector,
  TextArea,
  TextInput,
} from "@/ui/astryx"

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
  readonly loading: string
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
  readonly retry: string
  readonly retrying: string
  readonly status: Readonly<Record<TaskInspectorRelationTaskStatus, string>>
  readonly commentKindLabel: Readonly<Record<TaskInspectorCommentKind, string>>
  readonly copyCode: string
  readonly copiedCode: string
  readonly copyCodeError: string
}

const copy: Record<"zh" | "en", RelationsCopy> = {
  zh: {
    comments: "评论",
    dependencies: "依赖",
    steps: "步骤",
    newest: "最新优先",
    oldest: "最早优先",
    commentSortLabel: "评论排序",
    loading: "正在加载…",
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
    retry: "重试",
    retrying: "正在重试…",
    status: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    commentKindLabel: { note: "备注", decision: "决策", signal: "信号" },
    copyCode: "复制",
    copiedCode: "已复制",
    copyCodeError: "复制失败",
  },
  en: {
    comments: "Comments",
    dependencies: "Dependencies",
    steps: "Steps",
    newest: "Newest first",
    oldest: "Oldest first",
    commentSortLabel: "Comment sort order",
    loading: "Loading…",
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
    retry: "Retry",
    retrying: "Retrying…",
    status: { triage: "Triage", todo: "To do", scheduled: "Scheduled", ready: "Ready", running: "Running", blocked: "Blocked", review: "Review", done: "Done", archived: "Archived" },
    commentKindLabel: { note: "Note", decision: "Decision", signal: "Signal" },
    copyCode: "Copy",
    copiedCode: "Copied",
    copyCodeError: "Copy failed",
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

const writeOperations = [
  "saveTask",
  "transition",
  "addDependency",
  "removeDependency",
  "createStep",
  "linkStep",
  "markPlanNotRequired",
  "addLabel",
  "removeLabel",
  "applySuggestedLabel",
  "addComment",
  "uploadAttachment",
  "deleteAttachment",
] as const

function writePendingFor(snapshot: TaskInspectorMutationSnapshot, taskId: string): boolean {
  return pendingFor(snapshot, "reload", taskId) || writeOperations.some((operation) => pendingFor(snapshot, operation, taskId))
}

function useScopeEpoch(snapshot: TaskInspectorMutationSnapshot, taskId: string) {
  const identity = snapshot.scope.identity
  const generation = snapshot.generation
  const scopeEpochRef = useRef<InspectorScopeEpoch>({
    identity,
    generation,
    taskId,
  })
  const currentEpoch: InspectorScopeEpoch = {
    identity,
    generation,
    taskId,
  }
  const changed = !scopeEpochMatches(scopeEpochRef.current, currentEpoch)
  useLayoutEffect(() => {
    const committedEpoch: InspectorScopeEpoch = { identity, generation, taskId }
    if (!scopeEpochMatches(scopeEpochRef.current, committedEpoch)) scopeEpochRef.current = committedEpoch
  }, [generation, identity, taskId])
  return { ref: scopeEpochRef, changed }
}

interface RetryConfig {
  readonly key: string
  readonly handlers: TaskInspectorMutationHandlers
  readonly snapshot: TaskInspectorMutationSnapshot
  readonly disabled?: boolean
  readonly label: string
  readonly pendingLabel: string
  readonly onOutcome?: (outcome: InspectorMutationOutcome) => void
}

function RetryButton({ retry }: { readonly retry: RetryConfig }) {
  if (!retry.snapshot.retries.has(retry.key)) return null
  const pending = retry.snapshot.pending.has(retry.key)
  return (
    <Button
      type="button"
      data-testid={`task-inspector-retry-${retry.key}`}
      data-retry-key={retry.key}
      size="sm"
      variant="secondary"
      label={pending ? retry.pendingLabel : retry.label}
      isDisabled={retry.disabled || pending}
      isLoading={pending}
      onClick={() => {
        void retry.handlers.retry(retry.key)
          .then((outcome) => retry.onOutcome?.(outcome))
          .catch(() => undefined)
      }}
    />
  )
}

function Feedback({ error, pendingLabel, retry }: { readonly error: TaskInspectorMutationError | null; readonly pendingLabel?: string; readonly retry?: RetryConfig }) {
  return (
    <SafeVStack gap={2}>
      {pendingLabel ? <Text as="p" type="supporting" role="status" aria-live="polite">{pendingLabel}</Text> : null}
      {error ? (
        <Banner
          status="error"
          role="alert"
          aria-live="polite"
          title={error.code ? `${error.message} [${error.code}]` : error.message}
          container="section"
        />
      ) : null}
      {retry ? <RetryButton retry={retry} /> : null}
    </SafeVStack>
  )
}

function RelationTaskButton({ task, onSelectTask, localeCopy }: { readonly task: TaskInspectorRelationTaskView; readonly onSelectTask: (taskId: string) => void; readonly localeCopy: RelationsCopy }) {
  return (
    <Button
      type="button"
      size="sm"
      variant="ghost"
      label={`${task.ref} ${task.title}`}
      onClick={() => onSelectTask(task.id)}
      aria-label={`${task.ref} ${task.title}`}
    >
      <SafeHStack as="span" gap={2} align="center" wrap="wrap">
        <Text type="code" className="min-w-0 break-words">{task.ref}</Text>
        <Text type="body" wordBreak="break-word">{task.title}</Text>
        <Badge variant="neutral" label={localeCopy.status[task.status]} />
      </SafeHStack>
    </Button>
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
  const { ref: scopeEpochRef, changed: scopeChanged } = useScopeEpoch(snapshot, taskId)
  const renderEpoch = scopeEpochRef.current
  const draftRef = useRef({ kind, body })
  draftRef.current = { kind, body }
  const pageState = commentPageState(comments, page, pageSize, sortOrder)
  const pending = pendingFor(snapshot, "addComment", taskId)
  const writePending = writePendingFor(snapshot, taskId)
  const operationKey = inspectorMutationKey("addComment", taskId)
  const error = errorFor(snapshot, "addComment", taskId)
  const retryIntent = snapshot.retries.get(operationKey)
  const retryInput = retryIntent?.operation === "addComment" ? retryIntent.input : undefined
  const retryMatches = commentDraftMatchesRetry(kind, body, retryInput)

  useEffect(() => {
    if (!scopeChanged) return
    setSortOrder("newest")
    setPage(0)
    setKind("note")
    setBody("")
    setBodyError(null)
  }, [scopeChanged])

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const input = buildCommentInput(kind, body)
    if (!input.body) {
      setBodyError(localeCopy.requiredCommentBody)
      bodyRef.current?.focus()
      return
    }
    if (retryMatches) return
    const operationEpoch = scopeEpochRef.current
    try {
      const outcome = await handlers.addComment(input)
      if (!scopeEpochMatches(operationEpoch, scopeEpochRef.current)) return
      if (!shouldClearDraft(outcome, commentDraftMatchesRetry(draftRef.current.kind, draftRef.current.body, input))) return
      setBody("")
      setKind("note")
      setBodyError(null)
    } catch {
      // The shared controller owns the recoverable error snapshot. Keep draft text for retry.
    }
  }

  return (
    <SafeVStack as="section" gap={3} data-testid="task-inspector-comments">
      <SafeHStack as="header" justify="between" align="center" wrap="wrap" gap={2}>
        <Heading level={2}>{localeCopy.comments}</Heading>
        <Text type="supporting">{localeCopy.commentCount(comments.length)}</Text>
      </SafeHStack>
      {comments.length === 0 ? <Text as="p" type="supporting" role="status">{localeCopy.noComments}</Text> : (
        <SafeVStack gap={3}>
          <Selector
            id="task-inspector-comments-sort"
            data-testid="task-inspector-comments-sort"
            label={localeCopy.commentSortLabel}
            isLabelHidden
            options={[
              { value: "newest", label: localeCopy.newest },
              { value: "oldest", label: localeCopy.oldest },
            ]}
            value={sortOrder}
            onChange={(value) => {
              setSortOrder(value as CommentSortOrder)
              setPage(0)
            }}
            placeholder={localeCopy.commentSortLabel}
            loadingText={localeCopy.loading}
            htmlName="comments-sort"
          />
          <List density="compact" hasDividers>
            {pageState.comments.map((comment) => {
              const renderedTime = formatCommentDateTime(comment.createdAt, locale)
              return (
                <ListItem
                  key={comment.id}
                  label={(
                    <SafeHStack justify="between" align="center" gap={2} wrap="wrap">
                      <Text type="label" wordBreak="break-word">
                        {comment.author} · {localeCopy.commentKindLabel[comment.kind]}
                      </Text>
                      <time dateTime={renderedTime.iso || undefined}>{renderedTime.label}</time>
                    </SafeHStack>
                  )}
                  description={(
                    <SafeVStack gap={2}>
                      <Text as="p" type="body" wordBreak="break-word">{comment.body}</Text>
                      <Collapsible trigger={localeCopy.metadata} defaultIsOpen={false}>
                        <CodeBlock
                          code={safeJson(comment.metadata)}
                          language="json"
                          isWrapped
                          container="section"
                          maxHeight="compact"
                          label={localeCopy.metadata}
                          copyLabel={localeCopy.copyCode}
                          copiedLabel={localeCopy.copiedCode}
                          errorLabel={localeCopy.copyCodeError}
                        />
                      </Collapsible>
                    </SafeVStack>
                  )}
                />
              )
            })}
          </List>
          {pageState.pageCount > 1 ? (
            <SafeHStack justify="between" align="center" gap={2} wrap="wrap">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                label={localeCopy.previous}
                isDisabled={!pageState.hasPreviousPage}
                onClick={() => setPage((value) => Math.max(0, value - 1))}
              />
              <Text type="supporting" role="status" aria-live="polite">{localeCopy.page(pageState.page + 1, pageState.pageCount)}</Text>
              <Button
                data-testid="task-inspector-comments-next"
                type="button"
                size="sm"
                variant="secondary"
                label={localeCopy.next}
                isDisabled={!pageState.hasNextPage}
                onClick={() => setPage((value) => Math.min(pageState.pageCount - 1, value + 1))}
              />
            </SafeHStack>
          ) : null}
        </SafeVStack>
      )}
      <form onSubmit={(event) => void submit(event)} aria-busy={writePending || undefined}>
        <SafeVStack gap={2}>
          <Selector
            label={localeCopy.commentKind}
            options={[
              { value: "note", label: localeCopy.commentKindLabel.note },
              { value: "decision", label: localeCopy.commentKindLabel.decision },
              { value: "signal", label: localeCopy.commentKindLabel.signal },
            ]}
            value={kind}
            onChange={(value) => setKind(value as TaskInspectorCommentKind)}
            placeholder={localeCopy.commentKind}
            loadingText={localeCopy.loading}
            htmlName="comment-kind"
            isDisabled={writePending}
          />
          <TextArea
            ref={bodyRef}
            label={localeCopy.commentBody}
            value={body}
            onChange={(value) => { setBody(value); setBodyError(null) }}
            onInvalid={() => setBodyError(localeCopy.requiredCommentBody)}
            htmlName="comment-body"
            autoComplete="off"
            isRequired
            isDisabled={writePending}
            isLoading={pending}
            status={bodyError ? { type: "error" } : undefined}
            aria-describedby={bodyError ? "task-inspector-comment-body-error" : undefined}
            placeholder={localeCopy.commentPlaceholder}
            rows={3}
          />
          {bodyError ? <Banner id="task-inspector-comment-body-error" status="error" role="alert" title={bodyError} container="section" /> : null}
          <Button
            type="submit"
            size="sm"
            variant="primary"
            label={pending ? localeCopy.addingComment : localeCopy.addComment}
            isDisabled={writePending || retryMatches}
            isLoading={pending}
          />
          <Feedback
            error={error}
            pendingLabel={pending ? localeCopy.addingComment : undefined}
            retry={{
              key: operationKey,
              handlers,
              snapshot,
              disabled: writePending,
              label: localeCopy.retry,
              pendingLabel: localeCopy.retrying,
              onOutcome: (outcome) => {
                if (!scopeEpochMatches(renderEpoch, scopeEpochRef.current) || !shouldClearRetryDraft(outcome, commentDraftMatchesRetry(draftRef.current.kind, draftRef.current.body, retryInput))) return
                setBody("")
                setKind("note")
                setBodyError(null)
              },
            }}
          />
        </SafeVStack>
      </form>
    </SafeVStack>
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
  const { ref: scopeEpochRef, changed: scopeChanged } = useScopeEpoch(snapshot, taskId)
  const renderEpoch = scopeEpochRef.current
  const draftRef = useRef(input)
  draftRef.current = input
  const writePending = writePendingFor(snapshot, taskId)
  const addPending = pendingFor(snapshot, "addDependency", taskId)
  const addError = errorFor(snapshot, "addDependency", taskId)
  const addKey = inspectorMutationKey("addDependency", taskId)
  const removePending = pendingFor(snapshot, "removeDependency", taskId)
  const removeError = errorFor(snapshot, "removeDependency", taskId)
  const removeKey = inspectorMutationKey("removeDependency", taskId)
  const retryIntent = snapshot.retries.get(addKey)
  const retryParentTaskId = retryIntent?.operation === "addDependency" ? retryIntent.parentTaskId : undefined
  const retryMatches = dependencyDraftMatchesRetry(input, retryParentTaskId, resolveSelector)

  useEffect(() => {
    if (!scopeChanged) return
    setInput("")
    setResolutionError(null)
  }, [scopeChanged])

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
      inputRef.current?.focus()
      return
    }
    if (retryMatches) return
    const operationEpoch = scopeEpochRef.current
    try {
      const outcome = await handlers.addDependency(parentTaskId)
      if (!scopeEpochMatches(operationEpoch, scopeEpochRef.current)) return
      if (!shouldClearDraft(outcome, dependencyDraftMatchesRetry(draftRef.current, parentTaskId, resolveSelector))) return
      setInput("")
      setResolutionError(null)
    } catch {
      // Keep the ref/id visible so the controller can retry the same intent.
    }
  }

  const remove = async (parentTaskId: string) => {
    try {
      const outcome = await handlers.removeDependency(parentTaskId)
      if (!shouldClearDraft(outcome)) return
    } catch {
      // The shared controller owns the recoverable error snapshot.
    }
  }

  return (
    <SafeVStack as="section" gap={3} data-testid="task-inspector-dependencies">
      <Heading level={2}>{localeCopy.dependencies}</Heading>
      <SafeVStack as="section" gap={2}>
        <Heading level={3}>{localeCopy.parent}</Heading>
        {dependencies.parents.length === 0 ? <Text as="p" type="supporting" role="status">{localeCopy.noDependencies}</Text> : (
          <List density="compact" hasDividers>
            {dependencies.parents.map((task) => {
              const removeLabel = localeCopy.removeParent(task.title)
              return (
                <ListItem
                  key={task.id}
                  label={<RelationTaskButton task={task} onSelectTask={onSelectTask} localeCopy={localeCopy} />}
                  endContent={(
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      label={removePending ? localeCopy.removingDependency : removeLabel}
                      aria-label={removeLabel}
                      isDisabled={writePending}
                      isLoading={removePending}
                      onClick={() => void remove(task.id)}
                    />
                  )}
                />
              )
            })}
          </List>
        )}
      </SafeVStack>
      <SafeVStack as="section" gap={2}>
        <Heading level={3}>{localeCopy.child}</Heading>
        {dependencies.children.length === 0 ? <Text as="p" type="supporting" role="status">{localeCopy.noDependencies}</Text> : (
          <List density="compact" hasDividers>
            {dependencies.children.map((task) => (
              <ListItem key={task.id} label={<RelationTaskButton task={task} onSelectTask={onSelectTask} localeCopy={localeCopy} />} />
            ))}
          </List>
        )}
      </SafeVStack>
      <form onSubmit={(event) => void add(event)} aria-busy={writePending || undefined}>
        <SafeVStack gap={2}>
          <TextInput
            ref={inputRef}
            label={localeCopy.dependencyInput}
            value={input}
            onChange={(value) => { setInput(value); setResolutionError(null) }}
            onInvalid={() => setResolutionError(localeCopy.requiredDependency)}
            htmlName="dependency-parent"
            autoComplete="off"
            isRequired
            isDisabled={writePending}
            status={resolutionError ? { type: "error" } : undefined}
            aria-describedby={resolutionError ? "task-inspector-dependency-error" : undefined}
            placeholder={localeCopy.dependencyPlaceholder}
          />
          <Button
            type="submit"
            size="sm"
            variant="primary"
            label={addPending ? localeCopy.addingDependency : localeCopy.addDependency}
            isDisabled={writePending || retryMatches}
            isLoading={addPending}
          />
          <Feedback
            error={addError}
            pendingLabel={addPending ? localeCopy.addingDependency : undefined}
            retry={{
              key: addKey,
              handlers,
              snapshot,
              disabled: writePending,
              label: localeCopy.retry,
              pendingLabel: localeCopy.retrying,
              onOutcome: (outcome) => {
                if (!scopeEpochMatches(renderEpoch, scopeEpochRef.current) || !shouldClearRetryDraft(outcome, dependencyDraftMatchesRetry(draftRef.current, retryParentTaskId, resolveSelector))) return
                setInput("")
                setResolutionError(null)
              },
            }}
          />
          <Feedback
            error={removeError}
            pendingLabel={removePending ? localeCopy.removingDependency : undefined}
            retry={{ key: removeKey, handlers, snapshot, disabled: writePending, label: localeCopy.retry, pendingLabel: localeCopy.retrying }}
          />
          {resolutionError ? <Banner id="task-inspector-dependency-error" status="error" role="alert" aria-live="polite" title={resolutionError} container="section" /> : null}
        </SafeVStack>
      </form>
    </SafeVStack>
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
  const linkedTaskRefInput = useRef<HTMLInputElement | null>(null)
  const { ref: scopeEpochRef, changed: scopeChanged } = useScopeEpoch(snapshot, taskId)
  const renderEpoch = scopeEpochRef.current
  const draftRef = useRef({ title, body, required, linkedTaskRef, planReason })
  draftRef.current = { title, body, required, linkedTaskRef, planReason }
  const stepsView = asStepsView(stepsInput)
  const createPending = pendingFor(snapshot, "createStep", taskId)
  const createError = errorFor(snapshot, "createStep", taskId)
  const createKey = inspectorMutationKey("createStep", taskId)
  const linkPending = pendingFor(snapshot, "linkStep", taskId)
  const linkError = errorFor(snapshot, "linkStep", taskId)
  const linkKey = inspectorMutationKey("linkStep", taskId)
  const planPending = pendingFor(snapshot, "markPlanNotRequired", taskId)
  const planError = errorFor(snapshot, "markPlanNotRequired", taskId)
  const planKey = inspectorMutationKey("markPlanNotRequired", taskId)
  const stepPending = createPending || linkPending
  const writePending = writePendingFor(snapshot, taskId)
  const plan = stepsView.executionPlan
  const createRetryIntent = snapshot.retries.get(createKey)
  const createRetryInput = createRetryIntent?.operation === "createStep" ? createRetryIntent.input : undefined
  const linkRetryIntent = snapshot.retries.get(linkKey)
  const linkRetryInput = linkRetryIntent?.operation === "linkStep" ? linkRetryIntent.input : undefined
  const planRetryIntent = snapshot.retries.get(planKey)
  const planRetryReason = planRetryIntent?.operation === "markPlanNotRequired" ? planRetryIntent.input.reason : undefined
  const createRetryMatches = stepDraftMatchesRetry(title, body, required, linkedTaskRef, createRetryInput, resolveSelector)
  const linkRetryMatches = stepDraftMatchesRetry(title, body, required, linkedTaskRef, linkRetryInput, resolveSelector)
  const planRetryMatches = planDraftMatchesRetry(planReason, planRetryReason)

  useEffect(() => {
    if (!scopeChanged) return
    setTitle("")
    setBody("")
    setRequired(true)
    setLinkedTaskRef("")
    setLinkedTaskResolutionError(null)
    setTitleError(null)
    setPlanReason("")
    setPlanReasonError(null)
  }, [scopeChanged])

  const submitStep = async (link: boolean) => {
    if (!title.trim()) {
      setTitleError(localeCopy.requiredStepTitle)
      titleRef.current?.focus()
      return
    }
    const linkedTaskId = link && linkedTaskRef.trim() ? resolveTaskSelector(linkedTaskRef, resolveSelector) : null
    if (link && !linkedTaskId) {
      setLinkedTaskResolutionError(localeCopy.unresolvedLinkedTask)
      linkedTaskRefInput.current?.focus()
      return
    }
    const submission = buildStepSubmission(title, body, required, linkedTaskId ?? undefined, link ? "link" : "create")
    if (!submission) return
    if ((!link && createRetryMatches) || (link && linkRetryMatches)) return
    const operationEpoch = scopeEpochRef.current
    try {
      const outcome = submission.operation === "linkStep"
        ? await handlers.linkStep(submission.input)
        : await handlers.createStep(submission.input)
      if (!scopeEpochMatches(operationEpoch, scopeEpochRef.current)) return
      if (!shouldClearDraft(outcome, stepDraftMatchesRetry(draftRef.current.title, draftRef.current.body, draftRef.current.required, draftRef.current.linkedTaskRef, submission.input, resolveSelector))) return
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
    if (planRetryMatches) return
    const operationEpoch = scopeEpochRef.current
    try {
      const outcome = await handlers.markPlanNotRequired(input)
      if (!scopeEpochMatches(operationEpoch, scopeEpochRef.current)) return
      if (!shouldClearDraft(outcome, planDraftMatchesRetry(draftRef.current.planReason, input.reason))) return
      setPlanReason("")
      setPlanReasonError(null)
    } catch {
      // Keep the reason available for a retry.
    }
  }

  const clearStepDraftOnRetry = (outcome: InspectorMutationOutcome, expected: typeof createRetryInput | typeof linkRetryInput) => {
    if (!scopeEpochMatches(renderEpoch, scopeEpochRef.current) || !shouldClearRetryDraft(outcome, stepDraftMatchesRetry(draftRef.current.title, draftRef.current.body, draftRef.current.required, draftRef.current.linkedTaskRef, expected, resolveSelector))) return
    setTitle("")
    setBody("")
    setRequired(true)
    setLinkedTaskRef("")
    setLinkedTaskResolutionError(null)
    setTitleError(null)
  }

  const clearPlanDraftOnRetry = (outcome: InspectorMutationOutcome) => {
    if (!scopeEpochMatches(renderEpoch, scopeEpochRef.current) || !shouldClearRetryDraft(outcome, planDraftMatchesRetry(draftRef.current.planReason, planRetryReason))) return
    setPlanReason("")
    setPlanReasonError(null)
  }

  return (
    <SafeVStack as="section" gap={3} data-testid="task-inspector-steps">
      <SafeHStack as="header" justify="between" align="center" wrap="wrap" gap={2}>
        <Heading level={2}>{localeCopy.steps}</Heading>
        {plan ? <Badge variant="neutral" label={`${localeCopy.plan}: ${localeCopy.planState[plan.state]}`} /> : null}
      </SafeHStack>
      {stepsView.steps.length === 0 ? <Text as="p" type="supporting" role="status">{localeCopy.noSteps}</Text> : (
        <List density="compact" hasDividers>
          {stepsView.steps.map((step) => (
            <ListItem
              key={step.id}
              label={<Text type="label" wordBreak="break-word">{step.title}</Text>}
              description={(
                <SafeVStack gap={2}>
                  {step.body ? <Text as="p" type="body" wordBreak="break-word">{step.body}</Text> : null}
                  {step.linkedTask ? (
                    <SafeHStack gap={2} align="center" wrap="wrap">
                      <Text type="supporting">{localeCopy.linkedTask}</Text>
                      <RelationTaskButton task={step.linkedTask} onSelectTask={onSelectTask} localeCopy={localeCopy} />
                    </SafeHStack>
                  ) : null}
                </SafeVStack>
              )}
              endContent={<Badge variant="neutral" label={`${step.required ? localeCopy.required : localeCopy.optional} · ${localeCopy[step.status]}`} />}
            />
          ))}
        </List>
      )}
      <form onSubmit={(event) => { event.preventDefault(); void submitStep(false) }} aria-busy={writePending || undefined}>
        <SafeVStack gap={2}>
          <TextInput
            ref={titleRef}
            data-testid="task-inspector-step-title"
            label={localeCopy.stepTitle}
            value={title}
            onChange={(value) => { setTitle(value); setTitleError(null) }}
            onInvalid={() => setTitleError(localeCopy.requiredStepTitle)}
            htmlName="step-title"
            autoComplete="off"
            isRequired
            isDisabled={writePending}
            status={titleError ? { type: "error" } : undefined}
            aria-describedby={titleError ? "task-inspector-step-title-error" : undefined}
          />
          {titleError ? <Banner id="task-inspector-step-title-error" status="error" role="alert" aria-live="polite" title={titleError} container="section" /> : null}
          <TextArea
            label={localeCopy.stepBody}
            value={body}
            onChange={(value) => setBody(value)}
            htmlName="step-body"
            autoComplete="off"
            isDisabled={writePending}
            placeholder={localeCopy.stepBodyPlaceholder}
            rows={3}
          />
          <CheckboxInput
            label={localeCopy.required}
            value={required}
            onChange={(value) => setRequired(value)}
            htmlName="step-required"
            isDisabled={writePending}
            isLoading={stepPending}
            size="sm"
          />
          <TextInput
            ref={linkedTaskRefInput}
            label={localeCopy.stepLink}
            value={linkedTaskRef}
            onChange={(value) => { setLinkedTaskRef(value); setLinkedTaskResolutionError(null) }}
            htmlName="step-linked-task"
            autoComplete="off"
            isDisabled={writePending}
            aria-invalid={linkedTaskResolutionError ? "true" : "false"}
            aria-describedby={linkedTaskResolutionError ? "task-inspector-step-link-error" : undefined}
            placeholder={localeCopy.stepLinkPlaceholder}
          />
          <SafeHStack gap={2} wrap="wrap">
            <Button
              data-testid="task-inspector-create-step"
              type="button"
              size="sm"
              variant="primary"
              label={stepPending ? localeCopy.creatingStep : localeCopy.createStep}
              isDisabled={writePending || createRetryMatches}
              isLoading={createPending}
              onClick={() => void submitStep(false)}
            />
            <Button
              data-testid="task-inspector-link-step"
              type="button"
              size="sm"
              variant="secondary"
              label={stepPending ? localeCopy.creatingStep : localeCopy.createAndLinkStep}
              isDisabled={writePending || linkRetryMatches}
              isLoading={linkPending}
              onClick={() => void submitStep(true)}
            />
          </SafeHStack>
          <Feedback
            error={createError}
            pendingLabel={createPending ? localeCopy.creatingStep : undefined}
            retry={{ key: createKey, handlers, snapshot, disabled: writePending, label: localeCopy.retry, pendingLabel: localeCopy.retrying, onOutcome: (outcome) => clearStepDraftOnRetry(outcome, createRetryInput) }}
          />
          <Feedback
            error={linkError}
            pendingLabel={linkPending ? localeCopy.creatingStep : undefined}
            retry={{ key: linkKey, handlers, snapshot, disabled: writePending, label: localeCopy.retry, pendingLabel: localeCopy.retrying, onOutcome: (outcome) => clearStepDraftOnRetry(outcome, linkRetryInput) }}
          />
          {linkedTaskResolutionError ? <Banner id="task-inspector-step-link-error" status="error" role="alert" aria-live="polite" title={linkedTaskResolutionError} container="section" /> : null}
        </SafeVStack>
      </form>
      <form onSubmit={(event) => void submitPlan(event)} aria-busy={writePending || undefined}>
        <SafeVStack gap={2}>
          <TextInput
            ref={planReasonRef}
            label={localeCopy.planReason}
            value={planReason}
            onChange={(value) => { setPlanReason(value); setPlanReasonError(null) }}
            onInvalid={() => setPlanReasonError(localeCopy.requiredPlanReason)}
            htmlName="plan-not-required-reason"
            autoComplete="off"
            isRequired
            isDisabled={writePending}
            aria-invalid={planReasonError ? "true" : "false"}
            aria-describedby={planReasonError ? "task-inspector-plan-reason-error" : undefined}
            placeholder={localeCopy.planReasonPlaceholder}
          />
          {planReasonError ? <Banner id="task-inspector-plan-reason-error" status="error" role="alert" aria-live="polite" title={planReasonError} container="section" /> : null}
          <Button
            data-testid="task-inspector-mark-plan-not-required"
            type="submit"
            size="sm"
            variant="secondary"
            label={planPending ? localeCopy.markingPlanNotRequired : localeCopy.markPlanNotRequired}
            isDisabled={writePending || planRetryMatches || plan?.state === "not_required"}
            isLoading={planPending}
          />
          <Feedback
            error={planError}
            pendingLabel={planPending ? localeCopy.markingPlanNotRequired : undefined}
            retry={{ key: planKey, handlers, snapshot, disabled: writePending, label: localeCopy.retry, pendingLabel: localeCopy.retrying, onOutcome: clearPlanDraftOnRetry }}
          />
        </SafeVStack>
      </form>
    </SafeVStack>
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
    <SafeVStack as="section" gap={5} data-testid="task-inspector-relations">
      <CommentsPanel taskId={taskId} comments={comments} handlers={handlers} snapshot={snapshot} localeCopy={localeCopy} locale={locale} pageSize={safePageSize} />
      <DependenciesPanel taskId={taskId} dependencies={dependencies} handlers={handlers} snapshot={snapshot} onSelectTask={onSelectTask} resolveSelector={resolveSelector} localeCopy={localeCopy} />
      <StepsPanel taskId={taskId} stepsInput={steps} handlers={handlers} snapshot={snapshot} onSelectTask={onSelectTask} resolveSelector={resolveSelector} localeCopy={localeCopy} />
    </SafeVStack>
  )
}
