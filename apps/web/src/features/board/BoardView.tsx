import { useState } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"
import {
  PageFrame,
  SafeCard,
  SafeHStack,
  SafeVStack,
  Skeleton,
} from "@/ui/astryx"

import { taskOpenerKey } from "../../lib/explorer-focus"
import { attentionCounts, attentionLenses, type AttentionLens } from "../attention/attention-lens"
import { boardColumnsForAttention } from "./board-attention"
import { MutationDialog, MutationNotice } from "./BoardTaskMutations"
import {
  defaultBoardMessages,
  type BoardMessages,
  type BoardMessagesOverrides,
  type BoardTaskViewModel,
  type BoardViewModel,
  type BoardViewState,
  type BoardSyncStatus,
  validateBoardViewModel,
} from "./types"
import {
  canCompleteTask,
  canPromoteTask,
  transitionOptionsForTask,
  type BoardTaskMutationSurface,
} from "./task-mutation-state"
import {
  useBoardTaskMutationController,
  type BoardTaskMutationController,
} from "./task-mutation-controller"
import { BOARD_PAGE_SIZE, boardPageWindow } from "./board-pagination"

export interface BoardViewProps {
  readonly state: BoardViewState
  readonly messages?: BoardMessagesOverrides
  readonly onRetry?: () => void
  readonly onSelectTask?: (taskId: string) => void
  /** Sync state is rendered as an independent banner and never replaces a ready board. */
  readonly syncStatus?: BoardSyncStatus
  readonly id?: string
  readonly className?: string
  readonly taskMutations?: BoardTaskMutationSurface
  /** Explorer nests the live board under its page heading. */
  readonly headingLevel?: 1 | 2 | 3
  /** Canonical Tasks uses a compact projection; standalone consumers keep the full board header. */
  readonly presentation?: "standalone" | "embedded"
}

function mergeMessages(overrides?: BoardMessagesOverrides): BoardMessages {
  return {
    ...defaultBoardMessages,
    ...overrides,
    planState: {
      ...defaultBoardMessages.planState,
      ...overrides?.planState,
    },
  }
}

function priorityVariant(priority: BoardTaskViewModel["priority"]): "neutral" | "info" | "warning" | "error" {
  if (priority >= 3) return "error"
  if (priority === 2) return "warning"
  if (priority === 1) return "info"
  return "neutral"
}

const taskTimestampFormatters = new Map<string, Intl.DateTimeFormat>()

const visuallyHiddenClass = "sr-only"
const focusRingClass = "focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-1"
const mutedTextClass = "text-sm text-secondary"

function taskTimestampFormatter(locale: string): Intl.DateTimeFormat {
  const cached = taskTimestampFormatters.get(locale)
  if (cached) return cached
  try {
    const formatter = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" })
    taskTimestampFormatters.set(locale, formatter)
    return formatter
  } catch {
    const fallback = taskTimestampFormatters.get("en-US") ?? new Intl.DateTimeFormat("en-US", { dateStyle: "medium", timeStyle: "short" })
    taskTimestampFormatters.set("en-US", fallback)
    return fallback
  }
}

function taskTimestamp(value: number | null | undefined, locale: string): { readonly display: string; readonly iso: string } | null {
  if (value === null || value === undefined) return null
  const milliseconds = Math.abs(value) < 1_000_000_000_000 ? value * 1_000 : value
  const date = new Date(milliseconds)
  if (Number.isNaN(date.getTime())) return { display: String(value), iso: String(value) }
  return { display: taskTimestampFormatter(locale).format(date), iso: date.toISOString() }
}

function attentionCopy(copy: BoardMessages): {
  readonly label: string
  readonly statuses: Readonly<Record<AttentionLens, string>>
  readonly active: (label: string) => string
  readonly clear: string
  readonly noMatches: string
} {
  const english = copy.dateLocale.toLowerCase().startsWith("en")
  return english
    ? {
      label: "Attention",
      statuses: { ready: "Ready", running: "Running", blocked: "Blocked", review: "Review" },
      active: (label) => `Attention: ${label}`,
      clear: "Clear attention filter",
      noMatches: "No tasks match this attention filter.",
    }
    : {
      label: "关注入口",
      statuses: { ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核" },
      active: (label) => `当前关注：${label}`,
      clear: "清除关注筛选",
      noMatches: "没有任务符合当前关注筛选。",
    }
}

function columnAnchorId(rootId: string, index: number) {
  return `${rootId}-column-${index}`
}

function BoardHeader({
  board,
  titleId,
  copy,
  onCreate,
  isMutationPending,
  headingLevel,
}: {
  readonly board?: BoardViewModel["board"]
  readonly titleId: string
  readonly copy: BoardMessages
  readonly onCreate?: (trigger?: HTMLElement | null) => void
  readonly isMutationPending?: boolean
  readonly headingLevel: 1 | 2 | 3
}) {
  return (
    <SafeHStack as="div" justify="between" align="center" wrap="wrap" gap={3} className="min-w-0 border-b border-border pb-3">
      <SafeVStack gap={1} className="min-w-0 flex-1">
        <Heading level={headingLevel} id={titleId} className="min-w-0 break-words">
          {board?.name ?? copy.boardTitle}
        </Heading>
        {board ? (
          <SafeHStack as="div" wrap="wrap" align="center" gap={2} className="min-w-0 text-sm text-secondary">
            <Text as="span" type="supporting" weight="semibold">{copy.boardIdentityLabel}</Text>
            <code translate="no" data-testid="board-identity-slug">
              {board.slug}
            </code>
            <details className="inline-flex min-w-0 items-baseline gap-1" data-testid="board-identity-details">
              <summary onKeyDown={(event) => event.stopPropagation()}>ID</summary>
              <code translate="no">{board.id}</code>
            </details>
          </SafeHStack>
        ) : null}
      </SafeVStack>
      {onCreate ? <Button label={copy.createTask} variant="primary" isDisabled={isMutationPending} onClick={(event) => onCreate(event.currentTarget)} data-testid="task-create" /> : null}
    </SafeHStack>
  )
}

function EmptyBoard({ title, description }: { readonly title: string; readonly description: string }) {
  return (
    <SafeVStack as="section" gap={1} className="min-w-0 min-h-36 content-center border border-dashed border-border-strong bg-surface p-6" role="status" aria-live="polite" data-testid="board-empty">
      <Heading level={2} className="min-w-0 break-words">{title}</Heading>
      <Text as="p" type="supporting" className="max-w-prose min-w-0 break-words">{description}</Text>
    </SafeVStack>
  )
}

function SyncBanner({ status, copy, onRetry }: { readonly status: BoardSyncStatus; readonly copy: BoardMessages; readonly onRetry?: () => void }) {
  const title = status === "connecting"
    ? copy.syncConnecting
    : status === "live"
      ? copy.syncLive
      : status === "recovering"
        ? copy.syncRecovering
        : status === "circuit-open"
          ? copy.syncCircuitOpen
          : status === "offline"
            ? copy.offlineTitle
          : copy.syncStale
  const isHealthy = status === "live"
  return (
    <SafeHStack
      as="div"
      align="center"
      wrap="wrap"
      gap={2}
      className={`min-w-0 border-s-2 ${isHealthy ? "border-accent" : "border-warning"} bg-surface px-3 py-2 ${mutedTextClass}`}
      role="status"
      aria-live="polite"
      data-testid="board-sync-banner"
      data-sync-state={status}
    >
      <strong className="text-sm text-primary">{title}</strong>
      {!isHealthy ? <Text as="span" type="supporting">{copy.syncStaleDescription}</Text> : null}
      {!isHealthy && onRetry ? <Button label={copy.retry} variant="secondary" onClick={onRetry} /> : null}
    </SafeHStack>
  )
}

function StateContent({ state, copy, onRetry }: { readonly state: BoardViewState; readonly copy: BoardMessages; readonly onRetry?: () => void }) {
  if (state.kind === "loading") {
    return (
      <SafeVStack as="section" gap={2} className="min-w-0 min-h-32 content-center border border-border bg-surface p-5" role="status" aria-live="polite" data-testid="board-loading">
        <Skeleton size="row" className="max-w-sm" />
        <Text as="p" type="supporting">{copy.loading}</Text>
      </SafeVStack>
    )
  }

  if (state.kind === "empty") {
    const noBoards = state.board === undefined
    return (
      <EmptyBoard
        title={noBoards ? copy.noBoardsTitle : copy.emptyBoardTitle}
        description={state.detail ?? (noBoards ? copy.noBoardsDescription : copy.emptyBoardDescription)}
      />
    )
  }

  if (state.kind === "error" || state.kind === "offline") {
    return (
      <SafeVStack as="section" className="min-w-0 max-w-full" role="group" aria-label={state.kind === "error" ? copy.errorTitle : copy.offlineTitle}>
        <Banner
          status={state.kind === "error" ? "error" : "warning"}
          title={state.kind === "error" ? copy.errorTitle : copy.offlineTitle}
          description={state.message ?? copy.offlineTitle}
          endContent={onRetry ? <Button label={copy.retry} variant="secondary" onClick={onRetry} /> : undefined}
          data-testid={`board-${state.kind}`}
        />
      </SafeVStack>
    )
  }

  return null
}

function TaskCard({
  task,
  copy,
  controller,
  onSelectTask,
}: {
  readonly task: BoardTaskViewModel
  readonly copy: BoardMessages
  readonly controller?: BoardTaskMutationController
  readonly onSelectTask?: (taskId: string) => void
}) {
  const dependencyText = task.readiness.dependencyBlocked ? copy.dependencyBlocked : copy.dependencyClear
  const scheduledAt = taskTimestamp(task.scheduledAt, copy.dateLocale)
  const dueAt = taskTimestamp(task.dueAt, copy.dateLocale)
  const lastHeartbeatAt = taskTimestamp(task.lastHeartbeatAt, copy.dateLocale)
  const statusReason = task.statusReason?.trim() || copy.notAvailable
  const labels = task.labels ?? []
  const pending = controller?.isMutationPending === true
    || controller?.isPending(`transition:${task.id}`) === true
    || controller?.isPending(`edit:${task.id}`) === true
  const transitionOptions = controller ? transitionOptionsForTask(task, controller.claimTokenForTask(task.id)) : []
  const promoteNotReady = controller !== undefined && (task.status === "todo" || task.status === "scheduled") && !canPromoteTask(task)
  const requiredStepsIncomplete = controller !== undefined && (task.status === "running" || task.status === "review") && !canCompleteTask(task)

  return (
    <SafeCard
      ref={controller ? (element) => controller.onTaskRef(task.id, element) : undefined}
      className={`min-w-0 border-border-strong bg-surface shadow-none transition-colors hover:border-accent hover:bg-surface ${focusRingClass}`}
      padding={3}
      data-testid="board-task"
      data-task-id={task.id}
      data-status={task.status}
      draggable={controller ? !pending : undefined}
      tabIndex={controller ? 0 : undefined}
      role={controller ? "group" : undefined}
      aria-roledescription={controller ? copy.taskCardRoleDescription : undefined}
      aria-grabbed={controller ? controller.grabbedTaskId === task.id : undefined}
      aria-busy={pending || undefined}
      aria-label={controller ? `${task.ref} ${task.title}` : undefined}
      aria-keyshortcuts={controller ? "Space Escape ArrowLeft ArrowRight Enter" : undefined}
      onDragStart={controller ? (event) => controller.onDragStart(task.id, event) : undefined}
      onDragEnd={controller ? () => controller.onDragEnd(task.id) : undefined}
      onKeyDown={controller ? (event) => controller.onTaskKeyDown(task, event) : undefined}
    >
      <SafeHStack as="div" justify="between" align="start" gap={3} className="min-w-0">
        <code className="min-w-0 break-words text-xs" translate="no">{task.ref}</code>
        <Badge variant={priorityVariant(task.priority)} label={copy.priorityLabel(task.priority)} />
      </SafeHStack>
      <Heading level={3} className="mt-2 min-w-0 break-words text-base">
        {onSelectTask ? (
          <button type="button" className={`inline max-w-full break-words text-start text-inherit underline-offset-2 hover:text-accent hover:underline ${focusRingClass}`} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>
            {task.title}
          </button>
        ) : task.title}
      </Heading>
      <dl className="mt-2 grid min-w-0 gap-1 text-xs text-secondary" data-testid="board-task-summary">
        <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
          <dt>{copy.statusLabel}</dt>
          <dd className="m-0 min-w-0 break-words text-primary" translate="no" data-status={task.status}>{task.status}</dd>
        </SafeHStack>
        <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
          <dt>{copy.readinessLabel}</dt>
          <dd className="m-0 min-w-0 text-primary">
            <SafeHStack as="div" wrap="wrap" gap={2} className="min-w-0">
              <Text as="span" type="supporting" className="border-s-2 border-border-strong ps-2" data-testid="board-task-dependency">
                {copy.dependencyLabel}：{dependencyText}
              </Text>
              <Text as="span" type="supporting" className="border-s-2 border-border-strong ps-2" data-testid="board-task-unfinished-parents">
                {copy.unfinishedParentLabel}：{task.readiness.unfinishedParentCount}
              </Text>
              <Text as="span" type="supporting" className="border-s-2 border-border-strong ps-2" data-testid="board-task-required-steps">
                {copy.requiredStepsLabel}：{task.readiness.completedRequiredStepCount} / {task.readiness.requiredStepCount}
              </Text>
            </SafeHStack>
          </dd>
        </SafeHStack>
      </dl>
      <details className="mt-2 min-w-0 border-t border-border pt-2" data-testid="board-task-secondary">
        <summary onKeyDown={(event) => event.stopPropagation()}>
          {copy.taskDetailsLabel}
        </summary>
        <dl className="mt-2 grid min-w-0 gap-1 text-xs text-secondary">
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.assigneeLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary" data-testid="board-task-assignee">{task.assignee ?? copy.unassigned}</dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.planLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary" data-testid="board-task-plan">{copy.planState[task.readiness.executionPlanState]}</dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.statusReasonLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary" data-testid="board-task-status-reason">{statusReason}</dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.scheduledLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary" data-testid="board-task-scheduled">
              {scheduledAt ? <time dateTime={scheduledAt.iso}>{scheduledAt.display}</time> : copy.notAvailable}
            </dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.dueLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary" data-testid="board-task-due">
              {dueAt ? <time dateTime={dueAt.iso}>{dueAt.display}</time> : copy.notAvailable}
            </dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.lastHeartbeatLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary" data-testid="board-task-heartbeat">
              {lastHeartbeatAt ? <time dateTime={lastHeartbeatAt.iso}>{lastHeartbeatAt.display}</time> : copy.notAvailable}
            </dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.labelsLabel}</dt>
            <dd className="m-0 min-w-0 text-primary" data-testid="board-task-labels">
              {labels.length === 0 ? copy.noLabels : labels.map((label) => (
                <Text as="span" type="supporting" className="me-1 inline-flex max-w-full break-words rounded-md border border-border-strong bg-surface px-2 py-0.5" key={label.id} data-label-id={label.id}>{label.name}</Text>
              ))}
            </dd>
          </SafeHStack>
          <SafeHStack as="div" align="start" gap={2} className="grid min-w-0 grid-cols-2">
            <dt>{copy.optionalStepsLabel}</dt>
            <dd className="m-0 min-w-0 break-words text-primary">{task.readiness.optionalStepCount}</dd>
          </SafeHStack>
        </dl>
      </details>
      {controller ? (
        <details className="mt-3 min-w-0 border-t border-border pt-2" data-testid="board-task-actions">
          <summary onKeyDown={(event) => event.stopPropagation()}>{copy.transitionLabel}</summary>
          <SafeHStack as="div" wrap="wrap" gap={2} className="mt-2 min-w-0" role="group" aria-label={copy.transitionLabel} aria-busy={pending || undefined}>
            <Button
              label={copy.editTask}
              variant="secondary"
              size="sm"
              isDisabled={pending}
              onClick={(event) => controller.openEdit(task, event.currentTarget)}
              data-testid={`task-edit-${task.id}`}
            />
            {transitionOptions.map((option) => {
              const releaseDisabled = option.action === "release" && !controller.canReleaseTask(task.id)
              return (
                <Button
                  key={option.action}
                  label={pending ? copy.mutationPending : copy.transitionNames[option.action] ?? option.action}
                  variant="secondary"
                  size="sm"
                  isDisabled={pending || releaseDisabled}
                  tooltip={releaseDisabled ? copy.releaseClaimRequired : undefined}
                  onClick={(event) => controller.openTransition(task, option, event.currentTarget)}
                  data-testid={`task-transition-${option.action}-${task.id}`}
                />
              )
            })}
            {promoteNotReady ? <Text as="span" type="supporting" role="status" aria-live="polite">{copy.promoteNotReady}</Text> : null}
            {requiredStepsIncomplete ? <Text as="span" type="supporting" role="status" aria-live="polite">{copy.requiredStepsIncomplete}</Text> : null}
            {pending ? <Text as="span" type="supporting" role="status" aria-live="polite">{copy.mutationPending}</Text> : null}
          </SafeHStack>
        </details>
      ) : null}
    </SafeCard>
  )
}

function BoardColumns({
  model,
  copy,
  rootId,
  controller,
  onSelectTask,
  showColumnNavigation,
}: {
  readonly model: BoardViewModel
  readonly copy: BoardMessages
  readonly rootId: string
  readonly controller?: BoardTaskMutationController
  readonly onSelectTask?: (taskId: string) => void
  readonly showColumnNavigation: boolean
}) {
  const [pagesByColumn, setPagesByColumn] = useState<Record<string, number>>({})
  const [attentionLens, setAttentionLens] = useState<AttentionLens | null>(null)
  const baseColumnTasks = boardColumnsForAttention(model, null)
  const columns = baseColumnTasks.map(({ column }) => column)

  if (columns.length === 0) {
    const title = model.columns.length === 0 ? copy.emptyBoardTitle : copy.emptyVisibleColumnsTitle
    const description = model.columns.length === 0 ? copy.emptyBoardDescription : copy.emptyVisibleColumnsDescription
    return <EmptyBoard title={title} description={description} />
  }

  const copyForAttention = attentionCopy(copy)
  const allTasks = baseColumnTasks.flatMap(({ tasks }) => tasks)
  const counts = attentionCounts(allTasks)
  const columnTasks = boardColumnsForAttention(model, attentionLens)
  const boardTaskTotal = columnTasks.reduce((total, entry) => total + entry.tasks.length, 0)
  // Keep the full visible column set for all lenses. Filtering only the cards
  // preserves every mutation/drop target and keeps cross-column movement
  // reachable while still showing per-column empty states.
  const displayColumns = columnTasks

  return (
    <>
      <SafeHStack as="div" wrap="wrap" align="center" gap={2} className="min-w-0 border-b border-border pb-3" role="group" aria-label={copyForAttention.label} data-testid="board-attention-lens">
        <Text as="span" type="supporting" weight="semibold">{copyForAttention.label}</Text>
        {attentionLenses.map((lens) => (
          <button
            key={lens}
            type="button"
            className={`inline-flex min-h-8 items-center gap-1 rounded-full border border-border-strong bg-surface px-2 text-xs text-primary hover:border-accent ${focusRingClass} ${attentionLens === lens ? "border-accent bg-accent text-inverted" : ""}`}
            aria-pressed={attentionLens === lens}
            data-testid={`board-attention-${lens}`}
            onClick={() => setAttentionLens(attentionLens === lens ? null : lens)}
          >
            <Text as="span" type="supporting" color={attentionLens === lens ? "inherit" : undefined}>{copyForAttention.statuses[lens]}</Text>
            <Text as="span" type="supporting" color={attentionLens === lens ? "inherit" : undefined} className="min-w-5 rounded-full bg-muted px-1 text-center" data-testid={`board-attention-count-${lens}`}>{counts[lens]}</Text>
          </button>
        ))}
        {attentionLens !== null ? (
          <SafeHStack as="div" align="center" gap={2} className="min-w-0 text-xs text-secondary" data-testid="board-attention-active-filter">
            <Text as="span" type="supporting">{copyForAttention.active(copyForAttention.statuses[attentionLens])}</Text>
            <button type="button" className={`rounded-md border border-border-strong bg-surface px-2 py-1 text-xs text-primary hover:border-accent ${focusRingClass}`} data-testid="board-attention-clear" onClick={() => setAttentionLens(null)}>{copyForAttention.clear}</button>
          </SafeHStack>
        ) : null}
      </SafeHStack>
      <Text as="p" type="supporting" className="m-0 tabular-nums" data-testid="board-task-total" data-total={boardTaskTotal}>
        {copy.boardTaskTotal(boardTaskTotal)}
      </Text>
      {attentionLens !== null && boardTaskTotal === 0 ? (
        <SafeVStack as="section" gap={1} className="min-w-0 border border-border-strong bg-surface p-4" data-testid="board-attention-empty" role="status" aria-live="polite">
          <strong>{copyForAttention.noMatches}</strong>
          <Text as="span" type="supporting">{copyForAttention.active(copyForAttention.statuses[attentionLens])}</Text>
        </SafeVStack>
      ) : null}
      {showColumnNavigation ? (
        <nav className="min-w-0 overflow-x-auto overscroll-x-contain snap-x snap-mandatory touch-pan-x" aria-label={copy.columnNavigationLabel}>
          <ul className="flex min-w-max gap-3 m-0 list-none p-0">
            {displayColumns.map(({ column }, index) => (
              <li key={column.id}>
                <a className={`inline-flex min-h-8 items-center border-b border-border-strong px-1 text-sm text-secondary no-underline hover:border-accent hover:text-primary ${focusRingClass}`} href={`#${columnAnchorId(rootId, index)}`}>{column.title}</a>
              </li>
            ))}
          </ul>
        </nav>
      ) : null}
      <SafeVStack as="div" className={`boardColumns min-w-0 max-w-full overflow-x-auto overscroll-x-contain snap-x snap-mandatory touch-pan-x pb-2 ${focusRingClass}`} role="region" aria-label={copy.boardColumnsLabel} tabIndex={0}>
        <SafeHStack as="div" align="stretch" gap={2} className="min-w-max">
          {displayColumns.map(({ column, tasks }, index) => {
            const headingId = `${columnAnchorId(rootId, index)}-heading`
            const requestedPage = pagesByColumn[column.id] ?? 1
            const pageWindow = boardPageWindow(tasks.length, requestedPage)
            const visibleTasks = tasks.slice(pageWindow.start, pageWindow.end)
            const rangeStart = tasks.length === 0 ? 0 : pageWindow.start + 1
            const rangeEnd = pageWindow.end

            return (
              <section
                className={`grid w-72 min-w-60 snap-start content-start gap-2 rounded-md border border-border bg-muted ${focusRingClass}`}
                key={column.id}
                id={columnAnchorId(rootId, index)}
                aria-labelledby={headingId}
                tabIndex={-1}
                data-testid="board-column"
                data-column-id={column.id}
                data-status={column.status}
                onDragOver={controller ? (event) => controller.onDragOver(column.status, event) : undefined}
                onDrop={controller ? (event) => controller.onDrop(column.status, event) : undefined}
                aria-dropeffect={controller ? "move" : undefined}
                aria-label={controller ? `${column.title}；${copy.dropTargetLabel(column.title)}` : undefined}
              >
                <SafeVStack as="header" gap={1} className="min-w-0 border-b border-border px-3 py-2">
                  <Heading level={2} id={headingId} tabIndex={-1} className="min-w-0 break-words text-base">
                    {column.title}
                  </Heading>
                  <Text as="p" type="supporting" className="m-0 tabular-nums" data-testid="board-column-total" data-total={tasks.length}>
                    {copy.columnTaskCount(tasks.length)}
                  </Text>
                </SafeVStack>
                <ul className="m-0 grid min-w-0 gap-2 list-none px-2 pb-2" aria-label={column.title} data-testid={controller ? `board-drop-target-${column.status}` : undefined}>
                  {visibleTasks.length === 0 ? (
                    <li className="min-h-20 border border-dashed border-border p-4">
                      <p role="status" aria-live="polite">{copy.emptyColumn}</p>
                    </li>
                ) : (
                  visibleTasks.map((task) => (
                    <li className="min-w-0" key={task.id}>
                        <TaskCard task={task} copy={copy} controller={controller} onSelectTask={onSelectTask} />
                    </li>
                  ))
                )}
                </ul>
                {tasks.length > BOARD_PAGE_SIZE ? (
                  <SafeVStack
                    as="nav"
                    gap={2}
                    className="mx-2 mb-2 min-w-0 border-t border-border pt-3"
                    aria-label={copy.pageNavigationLabel(column.title)}
                    data-testid="board-column-pagination"
                    data-column-id={column.id}
                  >
                    <Text
                      as="span"
                      type="supporting"
                      className="tabular-nums"
                      aria-live="polite"
                      data-testid="board-column-page"
                      data-page={pageWindow.page}
                      data-total-pages={pageWindow.totalPages}
                      data-range-start={rangeStart}
                      data-range-end={rangeEnd}
                      data-total={tasks.length}
                    >
                      {copy.pageRange(rangeStart, rangeEnd, tasks.length)}
                    </Text>
                    <SafeHStack as="div" wrap="wrap" gap={2} className="min-w-0">
                      <button
                        type="button"
                        className={`min-h-9 rounded-md border border-border-strong bg-surface px-3 text-sm text-primary hover:border-accent disabled:cursor-not-allowed disabled:opacity-50 ${focusRingClass}`}
                        disabled={pageWindow.page <= 1}
                        aria-label={copy.pagePreviousLabel(column.title)}
                        data-testid="board-page-previous"
                        onClick={() => setPagesByColumn((current) => ({ ...current, [column.id]: pageWindow.page - 1 }))}
                      >
                        {copy.pagePrevious}
                      </button>
                      <button
                        type="button"
                        className={`min-h-9 rounded-md border border-border-strong bg-surface px-3 text-sm text-primary hover:border-accent disabled:cursor-not-allowed disabled:opacity-50 ${focusRingClass}`}
                        disabled={pageWindow.page >= pageWindow.totalPages}
                        aria-label={copy.pageNextLabel(column.title)}
                        data-testid="board-page-next"
                        onClick={() => setPagesByColumn((current) => ({ ...current, [column.id]: pageWindow.page + 1 }))}
                      >
                        {copy.pageNext}
                      </button>
                    </SafeHStack>
                  </SafeVStack>
                ) : null}
              </section>
            )
          })}
        </SafeHStack>
      </SafeVStack>
    </>
  )
}

export function BoardView({ state, messages: messageOverrides, onRetry, onSelectTask, syncStatus, id = "astryx-board", className, headingLevel = 1, taskMutations, presentation = "standalone" }: BoardViewProps) {
  const copy = mergeMessages(messageOverrides)
  const baseModel = state.kind === "ready" ? state.model : null
  const mutationColumns = baseModel !== null && Array.isArray(baseModel.columns) ? baseModel.columns : []
  const controller = useBoardTaskMutationController(baseModel, taskMutations, mutationColumns, copy)
  const displayModel = controller?.model ?? baseModel
  const titleId = `${id}-title`
  const validation = displayModel !== null ? validateBoardViewModel(displayModel) : { valid: true as const }
  const board =
    displayModel !== null && validation.valid ? displayModel.board : state.kind === "empty" ? state.board : undefined
  const renderedState: BoardViewState =
    displayModel !== null && !validation.valid
      ? { kind: "error", message: copy.invalidModelDescription }
      : state
  const rootClassName = [
    "grid min-w-0 max-w-full gap-3 text-primary",
    presentation === "embedded" ? "gap-2" : "",
    className ?? "",
  ].filter(Boolean).join(" ")
  const embeddedHeader = (
    <Heading level={headingLevel} id={titleId} className={visuallyHiddenClass}>
      {board?.name ?? copy.boardTitle}
    </Heading>
  )
  const embeddedToolbar = controller?.openCreate ? (
    <SafeHStack as="div" justify="end" align="center" gap={2} className="min-w-0">
      <Button
        label={copy.createTask}
        variant="primary"
        size="sm"
        isDisabled={controller.isMutationPending}
        onClick={(event) => controller.openCreate?.(event.currentTarget)}
        data-testid="task-create"
      />
    </SafeHStack>
  ) : null
  const hasSyncBanner = renderedState.kind === "ready" && syncStatus !== undefined
  const hasMutationNotice = controller !== null && controller.dialog === null && controller.notice !== null
  const syncAndNotices = hasSyncBanner || hasMutationNotice ? (
    <SafeVStack as="div" gap={2} className="min-w-0">
      {hasSyncBanner && syncStatus ? <SyncBanner status={syncStatus} copy={copy} onRetry={onRetry} /> : null}
      {hasMutationNotice ? <MutationNotice controller={controller} copy={copy} /> : null}
    </SafeVStack>
  ) : null
  const pageToolbar = presentation === "embedded"
    ? embeddedToolbar !== null || syncAndNotices !== null
      ? <SafeVStack as="div" gap={2} className="min-w-0">{embeddedToolbar}{syncAndNotices}</SafeVStack>
      : null
    : syncAndNotices
  const pageHeader = presentation === "embedded"
    ? embeddedHeader
    : <BoardHeader board={board} titleId={titleId} copy={copy} headingLevel={headingLevel} onCreate={controller?.openCreate} isMutationPending={controller?.isMutationPending} />
  const boardBody = (
    <>
      {controller?.isMutationPending ? <Text as="p" className={visuallyHiddenClass} type="supporting" role="status" aria-live="polite" data-testid="task-mutation-pending">{copy.mutationPending}</Text> : null}
      {controller ? <Text as="p" className={visuallyHiddenClass} type="supporting" role="status" aria-live="polite" data-testid="task-drag-announcement">{controller.dragAnnouncement}</Text> : null}
      <SafeVStack as="div" className="min-w-0 max-w-full" id={`${id}-columns`} tabIndex={-1}>
        {renderedState.kind === "ready" && displayModel !== null && validation.valid ? (
          <BoardColumns
            key={displayModel.board.id}
            model={displayModel}
            copy={copy}
            rootId={id}
            controller={controller ?? undefined}
            onSelectTask={onSelectTask}
            showColumnNavigation={presentation !== "embedded"}
          />
        ) : (
          <StateContent state={renderedState} copy={copy} onRetry={onRetry} />
        )}
      </SafeVStack>
    </>
  )
  const pageFrame = pageToolbar === null ? (
    <PageFrame frame="workspace" bodyLabel={copy.boardTitle} bodyOverflow="none" header={pageHeader}>
      {boardBody}
    </PageFrame>
  ) : (
    <PageFrame frame="workspace" bodyLabel={copy.boardTitle} bodyOverflow="none" header={pageHeader} toolbar={pageToolbar} toolbarLabel={copy.boardTitle}>
      {boardBody}
    </PageFrame>
  )

  return (
    <SafeVStack
      as="section"
      className={rootClassName}
      id={id}
      aria-labelledby={titleId}
      data-testid="board-view"
      data-state={renderedState.kind}
      data-anomaly={displayModel !== null && !validation.valid ? "board-model" : undefined}
      data-presentation={presentation}
      data-board-id={board?.id}
      data-board-slug={board?.slug}
    >
      <a className={`absolute z-10 -translate-y-full rounded-md border border-border-strong bg-surface px-3 py-2 text-primary focus:translate-y-0 ${focusRingClass}`} href={`#${id}-columns`}>
        {copy.skipToColumns}
      </a>
      {pageFrame}
      {controller ? <MutationDialog controller={controller} copy={copy} /> : null}
    </SafeVStack>
  )
}
