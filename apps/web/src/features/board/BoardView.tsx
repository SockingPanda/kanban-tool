import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"

import styles from "./BoardView.module.css"
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

export interface BoardViewProps {
  readonly state: BoardViewState
  readonly messages?: BoardMessagesOverrides
  readonly onRetry?: () => void
  readonly onSelectTask?: (taskId: string) => void
  /** Sync state is rendered as an independent banner and never replaces a ready board. */
  readonly syncStatus?: BoardSyncStatus
  readonly id?: string
  readonly className?: string
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
}: {
  readonly board?: BoardViewModel["board"]
  readonly titleId: string
  readonly copy: BoardMessages
}) {
  return (
    <header className={styles.header}>
      <div>
        <Text as="p" type="supporting" className={styles.eyebrow}>
          {copy.boardEyebrow}
        </Text>
        <Heading level={1} id={titleId}>
          {board?.name ?? copy.boardTitle}
        </Heading>
        {board ? (
          <p className={styles.identity}>
            <span className={styles.identityLabel}>{copy.boardIdentityLabel}</span>
            <code translate="no">
              {board.id} · {board.slug}
            </code>
          </p>
        ) : null}
      </div>
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

function TaskCard({ task, copy, onSelectTask }: { readonly task: BoardTaskViewModel; readonly copy: BoardMessages; readonly onSelectTask?: (taskId: string) => void }) {
  const dependencyText = task.readiness.dependencyBlocked
    ? `${copy.dependencyBlocked}（${task.readiness.unfinishedParentCount}）`
    : copy.dependencyClear

  return (
    <Card
      className={styles.taskCard}
      padding={3}
      data-testid="board-task"
      data-task-id={task.id}
      data-status={task.status}
    >
      <div className={styles.taskHeader}>
        <span className={styles.taskRef} translate="no">
          {task.ref}
        </span>
        <Badge variant={priorityVariant(task.priority)} label={copy.priorityLabel(task.priority)} />
      </div>
      <Heading level={3} className={styles.taskTitle}>
        {onSelectTask ? (
          <button type="button" className={styles.taskTitleButton} onClick={() => onSelectTask(task.id)}>
            {task.title}
          </button>
        ) : task.title}
      </Heading>
      <dl className={styles.taskDetails}>
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
            <span className={styles.readinessFact}>
              {copy.optionalStepsLabel}：{task.readiness.optionalStepCount}
            </span>
          </dd>
        </div>
      </dl>
    </Card>
  )
}

function BoardColumns({ model, copy, rootId, onSelectTask }: { readonly model: BoardViewModel; readonly copy: BoardMessages; readonly rootId: string; readonly onSelectTask?: (taskId: string) => void }) {
  const columns = orderedVisibleColumns(model.columns)

  if (columns.length === 0) {
    const title = model.columns.length === 0 ? copy.emptyBoardTitle : copy.emptyVisibleColumnsTitle
    const description = model.columns.length === 0 ? copy.emptyBoardDescription : copy.emptyVisibleColumnsDescription
    return <EmptyBoard title={title} description={description} />
  }

  return (
    <>
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
          {columns.map((column, index) => {
            const tasks = tasksForColumn(model, column)
            const headingId = `${columnAnchorId(rootId, index)}-heading`

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
              >
                <header className={styles.columnHeader}>
                  <Heading level={2} id={headingId} tabIndex={-1}>
                    {column.title}
                  </Heading>
                  <p className={styles.columnCount}>{copy.columnTaskCount(tasks.length)}</p>
                </header>
                <ul className={styles.taskList} aria-label={column.title}>
                  {tasks.length === 0 ? (
                    <li className={styles.emptyColumn} role="status" aria-live="polite">
                      <p>{copy.emptyColumn}</p>
                    </li>
                  ) : (
                    tasks.map((task) => (
                      <li className={styles.taskListItem} key={task.id}>
                        <TaskCard task={task} copy={copy} onSelectTask={onSelectTask} />
                      </li>
                    ))
                  )}
                </ul>
              </section>
            )
          })}
        </div>
      </div>
    </>
  )
}

export function BoardView({ state, messages: messageOverrides, onRetry, onSelectTask, syncStatus, id = "astryx-board", className }: BoardViewProps) {
  const copy = mergeMessages(messageOverrides)
  const titleId = `${id}-title`
  const validation = state.kind === "ready" ? validateBoardViewModel(state.model) : { valid: true as const }
  const board =
    state.kind === "ready" && validation.valid ? state.model.board : state.kind === "empty" ? state.board : undefined
  const renderedState: BoardViewState =
    state.kind === "ready" && !validation.valid
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
      data-anomaly={state.kind === "ready" && !validation.valid ? "board-model" : undefined}
    >
      <a className={styles.skipLink} href={`#${id}-columns`}>
        {copy.skipToColumns}
      </a>
      <BoardHeader board={board} titleId={titleId} copy={copy} />
      {renderedState.kind === "ready" && syncStatus ? <SyncBanner status={syncStatus} copy={copy} onRetry={onRetry} /> : null}
      <div id={`${id}-columns`} tabIndex={-1}>
        {renderedState.kind === "ready" ? (
          <BoardColumns model={renderedState.model} copy={copy} rootId={id} onSelectTask={onSelectTask} />
        ) : (
          <StateContent state={renderedState} copy={copy} onRetry={onRetry} />
        )}
      </div>
    </section>
  )
}
