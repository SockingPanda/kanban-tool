import { useState } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"

import { taskOpenerKey } from "../../lib/explorer-focus"
import styles from "./BoardView.module.css"
import { MutationDialog, MutationNotice } from "./BoardTaskMutations"
import {
  defaultBoardMessages,
  type BoardColumnViewModel,
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

function columnAnchorId(rootId: string, index: number) {
  return `${rootId}-column-${index}`
}

function orderedVisibleColumns(columns: readonly BoardColumnViewModel[]) {
  return columns
    .filter((column) => !column.hidden)
    .slice()
    .sort((left, right) => left.position - right.position)
}

function tasksForColumn(model: BoardViewModel, column: BoardColumnViewModel) {
  return (model.tasksByStatus[column.status] ?? []).slice().sort((left, right) => left.position - right.position)
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
    <header className={styles.header}>
      <div className={styles.headerIdentity}>
        <Heading level={headingLevel} id={titleId}>
          {board?.name ?? copy.boardTitle}
        </Heading>
        {board ? (
          <div className={styles.identity}>
            <span className={styles.identityLabel}>{copy.boardIdentityLabel}</span>
            <code translate="no" data-testid="board-identity-slug">
              {board.slug}
            </code>
            <details className={styles.identityDetails} data-testid="board-identity-details">
              <summary onKeyDown={(event) => event.stopPropagation()}>ID</summary>
              <code translate="no">{board.id}</code>
            </details>
          </div>
        ) : null}
      </div>
      {onCreate ? <Button label={copy.createTask} variant="primary" isDisabled={isMutationPending} onClick={(event) => onCreate(event.currentTarget)} data-testid="task-create" /> : null}
    </header>
  )
}

function EmptyBoard({ title, description }: { readonly title: string; readonly description: string }) {
  return (
    <div className={styles.emptyBoard} role="status" aria-live="polite" data-testid="board-empty">
      <Heading level={2}>{title}</Heading>
      <p>{description}</p>
    </div>
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
    <div
      className={`${styles.syncBanner} ${isHealthy ? styles.syncBannerLive : styles.syncBannerStale}`}
      role="status"
      aria-live="polite"
      data-testid="board-sync-banner"
      data-sync-state={status}
    >
      <strong>{title}</strong>
      {!isHealthy ? <span>{copy.syncStaleDescription}</span> : null}
      {!isHealthy && onRetry ? <Button label={copy.retry} variant="secondary" onClick={onRetry} /> : null}
    </div>
  )
}

function StateContent({ state, copy, onRetry }: { readonly state: BoardViewState; readonly copy: BoardMessages; readonly onRetry?: () => void }) {
  if (state.kind === "loading") {
    return (
      <div className={styles.statusPanel} role="status" aria-live="polite" data-testid="board-loading">
        <p className={styles.loadingMarker}>{copy.loading}</p>
      </div>
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
      <div className={styles.errorContainer}>
        <Banner
          status={state.kind === "error" ? "error" : "warning"}
          title={state.kind === "error" ? copy.errorTitle : copy.offlineTitle}
          description={<span className={styles.errorCopy}>{state.message ?? copy.offlineTitle}</span>}
          endContent={onRetry ? <Button label={copy.retry} variant="secondary" onClick={onRetry} /> : undefined}
          data-testid={`board-${state.kind}`}
        />
      </div>
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
  const dependencyText = task.readiness.dependencyBlocked
    ? `${copy.dependencyBlocked}（${task.readiness.unfinishedParentCount}）`
    : copy.dependencyClear
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
    <Card
      ref={controller ? (element) => controller.onTaskRef(task.id, element) : undefined}
      className={styles.taskCard}
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
      <div className={styles.taskHeader}>
        <span className={styles.taskRef} translate="no">
          {task.ref}
        </span>
        <Badge variant={priorityVariant(task.priority)} label={copy.priorityLabel(task.priority)} />
      </div>
      <Heading level={3} className={styles.taskTitle}>
        {onSelectTask ? (
          <button type="button" className={styles.taskTitleButton} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>
            {task.title}
          </button>
        ) : task.title}
      </Heading>
      <dl className={styles.taskSummary} data-testid="board-task-summary">
        <div className={styles.taskDetailsRow}>
          <dt>{copy.statusLabel}</dt>
          <dd translate="no">{task.status}</dd>
        </div>
        <div className={styles.taskDetailsRow}>
          <dt>{copy.assigneeLabel}</dt>
          <dd>{task.assignee ?? copy.unassigned}</dd>
        </div>
        <div className={styles.taskDetailsRow}>
          <dt>{copy.readinessLabel}</dt>
          <dd className={styles.readinessFacts}>
            <span className={styles.readinessFact}>
              {copy.dependencyLabel}：{dependencyText}
            </span>
            <span className={styles.readinessFact}>
              {copy.planLabel}：{copy.planState[task.readiness.executionPlanState]}
            </span>
            <span className={styles.readinessFact}>
              {copy.requiredStepsLabel}：{task.readiness.completedRequiredStepCount} / {task.readiness.requiredStepCount}
            </span>
          </dd>
        </div>
      </dl>
      <details className={styles.taskDetailsDisclosure} data-testid="board-task-secondary">
        <summary onKeyDown={(event) => event.stopPropagation()}>
          {copy.statusReasonLabel} · {copy.scheduledLabel} · {copy.labelsLabel}
        </summary>
        <dl className={styles.taskDetails}>
          <div className={styles.taskDetailsRow}>
            <dt>{copy.statusReasonLabel}</dt>
            <dd data-testid="board-task-status-reason">{statusReason}</dd>
          </div>
          <div className={styles.taskDetailsRow}>
            <dt>{copy.scheduledLabel}</dt>
            <dd data-testid="board-task-scheduled">
              {scheduledAt ? <time dateTime={scheduledAt.iso}>{scheduledAt.display}</time> : copy.notAvailable}
            </dd>
          </div>
          <div className={styles.taskDetailsRow}>
            <dt>{copy.dueLabel}</dt>
            <dd data-testid="board-task-due">
              {dueAt ? <time dateTime={dueAt.iso}>{dueAt.display}</time> : copy.notAvailable}
            </dd>
          </div>
          <div className={styles.taskDetailsRow}>
            <dt>{copy.lastHeartbeatLabel}</dt>
            <dd data-testid="board-task-heartbeat">
              {lastHeartbeatAt ? <time dateTime={lastHeartbeatAt.iso}>{lastHeartbeatAt.display}</time> : copy.notAvailable}
            </dd>
          </div>
          <div className={styles.taskDetailsRow}>
            <dt>{copy.labelsLabel}</dt>
            <dd className={styles.taskLabels} data-testid="board-task-labels">
              {labels.length === 0 ? copy.noLabels : labels.map((label) => (
                <span className={styles.taskLabel} key={label.id} data-label-id={label.id}>{label.name}</span>
              ))}
            </dd>
          </div>
          <div className={styles.taskDetailsRow}>
            <dt>{copy.optionalStepsLabel}</dt>
            <dd>{task.readiness.optionalStepCount}</dd>
          </div>
        </dl>
      </details>
      {controller ? (
        <details className={styles.taskActionsDisclosure} data-testid="board-task-actions">
          <summary onKeyDown={(event) => event.stopPropagation()}>{copy.transitionLabel}</summary>
          <div className={styles.taskActions} role="group" aria-label={copy.transitionLabel} aria-busy={pending || undefined}>
            <Button
              label={copy.editTask}
              variant="secondary"
              size="sm"
              isDisabled={pending}
              onClick={(event) => controller.openEdit(task, event.currentTarget)}
              data-testid={`task-edit-${task.id}`}
            />
            {transitionOptions.map((option) => (
              <Button
                key={option.action}
                label={pending ? copy.mutationPending : copy.transitionNames[option.action] ?? option.action}
                variant="secondary"
                size="sm"
                isDisabled={pending}
                isLoading={pending}
                onClick={(event) => controller.openTransition(task, option, event.currentTarget)}
                data-testid={`task-transition-${option.action}-${task.id}`}
              />
            ))}
            {promoteNotReady ? <span role="status" aria-live="polite" className={styles.mutedAction}>{copy.promoteNotReady}</span> : null}
            {requiredStepsIncomplete ? <span role="status" aria-live="polite" className={styles.mutedAction}>{copy.requiredStepsIncomplete}</span> : null}
            {pending ? <span role="status" aria-live="polite" className={styles.mutedAction}>{copy.mutationPending}</span> : null}
          </div>
        </details>
      ) : null}
    </Card>
  )
}

function BoardColumns({
  model,
  copy,
  rootId,
  controller,
  onSelectTask,
}: {
  readonly model: BoardViewModel
  readonly copy: BoardMessages
  readonly rootId: string
  readonly controller?: BoardTaskMutationController
  readonly onSelectTask?: (taskId: string) => void
}) {
  const [pagesByColumn, setPagesByColumn] = useState<Record<string, number>>({})
  const columns = orderedVisibleColumns(model.columns)

  if (columns.length === 0) {
    const title = model.columns.length === 0 ? copy.emptyBoardTitle : copy.emptyVisibleColumnsTitle
    const description = model.columns.length === 0 ? copy.emptyBoardDescription : copy.emptyVisibleColumnsDescription
    return <EmptyBoard title={title} description={description} />
  }

  const columnTasks = columns.map((column) => ({ column, tasks: tasksForColumn(model, column) }))
  const boardTaskTotal = columnTasks.reduce((total, entry) => total + entry.tasks.length, 0)

  return (
    <>
      <p className={styles.boardTotal} data-testid="board-task-total" data-total={boardTaskTotal}>
        {copy.boardTaskTotal(boardTaskTotal)}
      </p>
      <nav className={styles.columnNavigation} aria-label={copy.columnNavigationLabel}>
        <ul className={styles.columnNavigationList}>
          {columns.map((column, index) => (
            <li key={column.id}>
              <a href={`#${columnAnchorId(rootId, index)}`}>{column.title}</a>
            </li>
          ))}
        </ul>
      </nav>
      <div className={styles.boardColumns} role="region" aria-label={copy.boardColumnsLabel} tabIndex={0}>
        <div className={styles.columnsGrid}>
          {columnTasks.map(({ column, tasks }, index) => {
            const headingId = `${columnAnchorId(rootId, index)}-heading`
            const requestedPage = pagesByColumn[column.id] ?? 1
            const pageWindow = boardPageWindow(tasks.length, requestedPage)
            const visibleTasks = tasks.slice(pageWindow.start, pageWindow.end)
            const rangeStart = tasks.length === 0 ? 0 : pageWindow.start + 1
            const rangeEnd = pageWindow.end

            return (
              <section
                className={styles.column}
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
                <header className={styles.columnHeader}>
                  <Heading level={2} id={headingId} tabIndex={-1}>
                    {column.title}
                  </Heading>
                  <p className={styles.columnCount} data-testid="board-column-total" data-total={tasks.length}>
                    {copy.columnTaskCount(tasks.length)}
                  </p>
                </header>
                <ul className={styles.taskList} aria-label={column.title} data-testid={controller ? `board-drop-target-${column.status}` : undefined}>
                  {visibleTasks.length === 0 ? (
                    <li className={styles.emptyColumn}>
                      <p role="status" aria-live="polite">{copy.emptyColumn}</p>
                    </li>
                ) : (
                  visibleTasks.map((task) => (
                    <li className={styles.taskListItem} key={task.id}>
                        <TaskCard task={task} copy={copy} controller={controller} onSelectTask={onSelectTask} />
                    </li>
                  ))
                )}
                </ul>
                {tasks.length > BOARD_PAGE_SIZE ? (
                  <nav
                    className={styles.pagination}
                    aria-label={copy.pageNavigationLabel(column.title)}
                    data-testid="board-column-pagination"
                    data-column-id={column.id}
                  >
                    <span
                      className={styles.pageRange}
                      aria-live="polite"
                      data-testid="board-column-page"
                      data-page={pageWindow.page}
                      data-total-pages={pageWindow.totalPages}
                      data-range-start={rangeStart}
                      data-range-end={rangeEnd}
                      data-total={tasks.length}
                    >
                      {copy.pageRange(rangeStart, rangeEnd, tasks.length)}
                    </span>
                    <div className={styles.pageActions}>
                      <button
                        type="button"
                        className={styles.pageButton}
                        disabled={pageWindow.page <= 1}
                        aria-label={copy.pagePreviousLabel(column.title)}
                        data-testid="board-page-previous"
                        onClick={() => setPagesByColumn((current) => ({ ...current, [column.id]: pageWindow.page - 1 }))}
                      >
                        {copy.pagePrevious}
                      </button>
                      <button
                        type="button"
                        className={styles.pageButton}
                        disabled={pageWindow.page >= pageWindow.totalPages}
                        aria-label={copy.pageNextLabel(column.title)}
                        data-testid="board-page-next"
                        onClick={() => setPagesByColumn((current) => ({ ...current, [column.id]: pageWindow.page + 1 }))}
                      >
                        {copy.pageNext}
                      </button>
                    </div>
                  </nav>
                ) : null}
              </section>
            )
          })}
        </div>
      </div>
    </>
  )
}

export function BoardView({ state, messages: messageOverrides, onRetry, onSelectTask, syncStatus, id = "astryx-board", className, headingLevel = 1, taskMutations }: BoardViewProps) {
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
  const rootClassName = className ? `${styles.board} ${className}` : styles.board

  return (
    <section
      className={rootClassName}
      id={id}
      aria-labelledby={titleId}
      data-testid="board-view"
      data-state={renderedState.kind}
      data-anomaly={displayModel !== null && !validation.valid ? "board-model" : undefined}
    >
      <a className={styles.skipLink} href={`#${id}-columns`}>
        {copy.skipToColumns}
      </a>
      <BoardHeader board={board} titleId={titleId} copy={copy} headingLevel={headingLevel} onCreate={controller?.openCreate} isMutationPending={controller?.isMutationPending} />
      {renderedState.kind === "ready" && syncStatus ? <SyncBanner status={syncStatus} copy={copy} onRetry={onRetry} /> : null}
      {controller && controller.dialog === null ? <MutationNotice controller={controller} copy={copy} /> : null}
      {controller?.isMutationPending ? <p className={styles.visuallyHidden} role="status" aria-live="polite" data-testid="task-mutation-pending">{copy.mutationPending}</p> : null}
      {controller ? <p className={styles.visuallyHidden} role="status" aria-live="polite" data-testid="task-drag-announcement">{controller.dragAnnouncement}</p> : null}
      <div className={styles.boardContent} id={`${id}-columns`} tabIndex={-1}>
        {renderedState.kind === "ready" && displayModel !== null && validation.valid ? (
          <BoardColumns key={displayModel.board.id} model={displayModel} copy={copy} rootId={id} controller={controller ?? undefined} onSelectTask={onSelectTask} />
        ) : (
          <StateContent state={renderedState} copy={copy} onRetry={onRetry} />
        )}
      </div>
      {controller ? <MutationDialog controller={controller} copy={copy} /> : null}
    </section>
  )
}
