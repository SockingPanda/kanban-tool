import { useId } from "react"

import type { BoardTaskStatus, BoardTaskViewModel } from "../../features/board/types"
import type { TaskInspectorViewModel } from "../../features/explorer/TaskInspector"
import styles from "./tasks.module.css"
import type {
  BoardColumnProps,
  DisplayMenuProps,
  SidePeekFrameProps,
  TaskCardProps,
  TaskFilterBarProps,
  TaskStateBoundaryProps,
  TaskTableProps,
  TasksDensity,
  TasksView,
} from "./tasks.types"

const statusLabels: Readonly<Record<BoardTaskStatus, string>> = {
  triage: "分诊",
  todo: "待办",
  scheduled: "已排期",
  ready: "就绪",
  running: "运行中",
  blocked: "已阻塞",
  review: "待审核",
  done: "已完成",
  archived: "已归档",
}

const statusLabelsEnglish: Readonly<Record<BoardTaskStatus, string>> = {
  triage: "Triage",
  todo: "To do",
  scheduled: "Scheduled",
  ready: "Ready",
  running: "Running",
  blocked: "Blocked",
  review: "Review",
  done: "Done",
  archived: "Archived",
}

function statusClass(status: BoardTaskStatus): string {
  if (status === "ready") return styles.statusReady
  if (status === "running") return styles.statusRunning
  if (status === "blocked") return styles.statusBlocked
  if (status === "review" || status === "scheduled") return styles.statusReview
  return ""
}

function densityClass(density: TasksDensity): string {
  return density === "comfortable" ? styles.cardComfortable : styles.cardDense
}

function stepsText(task: Pick<BoardTaskViewModel, "readiness">, locale: "zh" | "en" = "zh"): string {
  const { completedRequiredStepCount, requiredStepCount, optionalStepCount } = task.readiness
  return `${completedRequiredStepCount}/${requiredStepCount}${optionalStepCount > 0 ? ` + ${optionalStepCount}` : ""} ${locale === "en" ? "steps" : "步骤"}`
}

function taskStatusLabel(status: BoardTaskStatus, locale: "zh" | "en"): string {
  return (locale === "en" ? statusLabelsEnglish : statusLabels)[status] ?? status
}

function CloseIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="m3.5 3.5 9 9M12.5 3.5l-9 9" />
    </svg>
  )
}

export interface ViewSwitcherProps {
  readonly activeView: TasksView
  readonly onViewChange?: (view: TasksView) => void
  readonly includeUnsupportedTimeline?: boolean
  readonly label?: string
}

export function ViewSwitcher({ activeView, onViewChange, includeUnsupportedTimeline = false, label = "任务视图" }: ViewSwitcherProps) {
  const views: readonly { readonly id: TasksView; readonly label: string; readonly unsupported?: boolean }[] = [
    { id: "board", label: "Board" },
    { id: "list", label: "List" },
    { id: "table", label: "Table" },
    { id: "map", label: "Map" },
    ...(includeUnsupportedTimeline ? [{ id: "timeline" as const, label: "Timeline", unsupported: true }] : []),
  ]

  return (
    <div className={styles.viewSwitcher} role="group" aria-label={label}>
      {views.map((view) => (
        <button
          key={view.id}
          type="button"
          className={styles.viewButton}
          aria-pressed={activeView === view.id}
          aria-disabled={view.unsupported ? true : undefined}
          disabled={view.unsupported}
          title={view.unsupported ? "Timeline 需要 canonical timeline read model" : undefined}
          onClick={() => {
            if (!view.unsupported) onViewChange?.(view.id)
          }}
        >
          {view.label}
          {view.unsupported ? "（不可用）" : ""}
        </button>
      ))}
    </div>
  )
}

export function ActiveFilter({ label, onRemove, removeLabel = "移除筛选" }: { readonly label: string; readonly onRemove?: () => void; readonly removeLabel?: string }) {
  return (
    <span className={styles.chip}>
      <span>{label}</span>
      {onRemove ? (
        <button type="button" className={styles.chipRemove} aria-label={`${removeLabel}：${label}`} onClick={onRemove}>
          <CloseIcon />
        </button>
      ) : null}
    </span>
  )
}

export function FilterBar({
  search = "",
  placeholder = "搜索标题或 ref",
  filters = [],
  onSearchChange,
  onRemoveFilter,
  onClearFilters,
  onOpenFilters,
  filterButtonLabel = "Filters",
  clearButtonLabel = "清除筛选",
}: TaskFilterBarProps) {
  const labelId = useId()
  return (
    <div className={styles.filterBar} role="search" aria-label="任务筛选">
      <label className={styles.searchField} htmlFor={labelId}>
        <span>Search</span>
        <input
          id={labelId}
          type="search"
          value={search}
          placeholder={placeholder}
          onChange={(event) => onSearchChange?.(event.currentTarget.value)}
        />
      </label>
      <button type="button" className={styles.filterButton} onClick={onOpenFilters}>{filterButtonLabel}</button>
      {filters.length > 0 ? (
        <div className={styles.activeFilters} aria-label="Active filters">
          <span className={styles.activeFiltersLabel}>Active</span>
          {filters.map((filter) => (
            <ActiveFilter
              key={filter.id}
              label={filter.label}
              onRemove={filter.removable === false ? undefined : onRemoveFilter ? () => onRemoveFilter(filter.id) : undefined}
            />
          ))}
          {onClearFilters ? <button type="button" className={styles.clearButton} onClick={onClearFilters}>{clearButtonLabel}</button> : null}
        </div>
      ) : null}
    </div>
  )
}

export function DisplayMenu({ options, defaultOpen, open, onOpenChange, onDensityChange, onColumnVisibilityChange, columns = [], label = "Display" }: DisplayMenuProps) {
  return (
    <details className={styles.displayMenu} open={open ?? defaultOpen} onToggle={(event) => onOpenChange?.(event.currentTarget.open)}>
      <summary className={styles.displaySummary}>{label}</summary>
      <div className={styles.displayPanel} role="group" aria-label={label}>
        <p className={styles.displayHeading}>Density</p>
        <div className={styles.densityChoices}>
          {(["dense", "comfortable"] as const).map((density) => (
            <button
              key={density}
              type="button"
              className={styles.densityButton}
              aria-pressed={options.density === density}
              onClick={() => onDensityChange?.(density)}
            >
              {density === "dense" ? "Dense" : "Comfortable"}
            </button>
          ))}
        </div>
        {columns.length > 0 ? (
          <fieldset className={styles.columnToggles}>
            <legend className={styles.displayHeading}>Visible fields</legend>
            {columns.map((column) => {
              const checked = options.visibleColumns?.[column.id] ?? true
              return (
                <label key={column.id} className={styles.columnToggle}>
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={(event) => onColumnVisibilityChange?.(column.id, event.currentTarget.checked)}
                  />
                  <span>{column.label}</span>
                </label>
              )
            })}
          </fieldset>
        ) : null}
      </div>
    </details>
  )
}

export function TaskCard({ task, selected = false, density = "dense", locale = "zh", statusLabel, onSelect }: TaskCardProps) {
  const label = statusLabel ?? taskStatusLabel(task.status, locale)
  return (
    <button
      type="button"
      className={`${styles.card} ${densityClass(density)}`}
      aria-pressed={selected}
      aria-label={`${task.ref} ${task.title}`}
      onClick={() => onSelect?.(task)}
    >
      <span className={styles.cardMeta}>
        <span className={styles.cardRef}>{task.ref}</span>
        <span className={`${styles.status} ${statusClass(task.status)}`}>{label}</span>
      </span>
      <span className={styles.cardTitle}>{task.title}</span>
      <span className={styles.cardFooter}>
        {task.readiness.dependencyBlocked ? <span className={styles.dependency}>{locale === "en" ? "Waiting on dependency" : "等待依赖"}</span> : <span />}
        <span className={styles.steps}>{stepsText(task, locale)}</span>
      </span>
    </button>
  )
}

export function BoardColumn({ id, status, title, tasks, selectedTaskId = null, density = "dense", onSelectTask, emptyLabel = "暂无任务" }: BoardColumnProps) {
  const headingId = `${id}-heading`
  return (
    <section className={styles.column} aria-labelledby={headingId} data-status={status}>
      <header className={styles.columnHeader}>
        <h2 id={headingId} className={styles.columnTitle}>{title}</h2>
        <span className={styles.columnCount} aria-label={`${tasks.length} 个任务`}>{tasks.length}</span>
      </header>
      <div className={styles.columnBody}>
        {tasks.length > 0 ? tasks.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            density={density}
            selected={selectedTaskId === task.id}
            onSelect={onSelectTask}
          />
        )) : <p className={styles.columnEmpty}>{emptyLabel}</p>}
      </div>
    </section>
  )
}

export function BoardColumns({ columns, density = "dense", selectedTaskId, onSelectTask }: { readonly columns: readonly BoardColumnProps[]; readonly density?: TasksDensity; readonly selectedTaskId?: string | null; readonly onSelectTask?: (task: BoardTaskViewModel) => void }) {
  return (
    <div className={styles.boardScroll}>
      <div className={styles.board} role="region" aria-label="任务看板">
        {columns.map((column) => <BoardColumn key={column.id} {...column} density={density} selectedTaskId={selectedTaskId} onSelectTask={onSelectTask} />)}
      </div>
    </div>
  )
}

export function TaskTable({ tasks, selectedTaskId = null, density = "dense", locale = "zh", onSelectTask, caption }: TaskTableProps) {
  const tableClass = density === "comfortable" ? styles.tableComfortable : styles.tableDense
  const tableCaption = caption ?? (locale === "en" ? "Task table" : "任务表格")
  return (
    <div className={styles.tableWrap} role="region" aria-label={tableCaption} tabIndex={0}>
      <table className={`${styles.table} ${tableClass}`}>
        <caption className={styles.visuallyHidden}>{tableCaption}</caption>
        <thead>
          <tr>
            <th scope="col">Ref</th>
            <th scope="col">{locale === "en" ? "Title" : "标题"}</th>
            <th scope="col">{locale === "en" ? "Status" : "状态"}</th>
            <th scope="col">{locale === "en" ? "Priority" : "优先级"}</th>
            <th scope="col">{locale === "en" ? "Steps" : "步骤"}</th>
            <th scope="col">{locale === "en" ? "Assignee" : "执行者"}</th>
          </tr>
        </thead>
        <tbody>
          {tasks.map((task) => (
            <tr key={task.id} aria-selected={selectedTaskId === task.id}>
              <td className={styles.tableRef}>{task.ref}</td>
              <td>
                <button type="button" className={styles.tableTask} onClick={() => onSelectTask?.(task)}>{task.title}</button>
              </td>
              <td><span className={`${styles.status} ${statusClass(task.status)}`}>{taskStatusLabel(task.status, locale)}</span></td>
              <td>P{task.priority}</td>
              <td className={styles.steps}>{stepsText(task, locale)}</td>
              <td>{task.assignee ?? "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

const stateDefaults: Readonly<Record<TaskStateBoundaryProps["state"], { readonly title: string; readonly detail: string }>> = {
  loading: { title: "正在加载任务", detail: "正在读取 canonical board 数据。" },
  empty: { title: "暂无任务", detail: "这个 board 当前没有可展示的任务。" },
  offline: { title: "当前离线", detail: "保留最近一次任务数据；联网后可以重试。" },
  stale: { title: "数据可能已过期", detail: "实时连接中断，当前内容仍可查看。" },
  recovering: { title: "正在恢复连接", detail: "正在重新建立 board session。" },
  error: { title: "任务加载失败", detail: "读取任务时发生错误，请重试。" },
}

export function TaskStateBoundary({ state, title, detail, actionLabel, onAction, children }: TaskStateBoundaryProps) {
  const fallback = stateDefaults[state]
  const role = state === "error" ? "alert" : "status"
  return (
    <section className={styles.state} data-state={state} role={role} aria-live="polite">
      <h2 className={styles.stateTitle}>{title ?? fallback.title}</h2>
      <p className={styles.stateDetail}>{detail ?? fallback.detail}</p>
      {children}
      {onAction ? <button type="button" className={styles.stateAction} onClick={onAction}>{actionLabel ?? "重试"}</button> : null}
    </section>
  )
}

export function SidePeekFrame({
  model,
  locale = "zh",
  onClose,
  onOpenDetails,
  closeLabel = "关闭任务详情",
  detailsLabel = "打开完整详情",
  statusLabel = "状态",
  requiredStepLabel = "必需步骤",
  runLabel = "运行",
}: SidePeekFrameProps) {
  const task = model.task
  const titleId = useId()
  const requiredStep = model.steps.find((step) => step.required)
  return (
    <aside className={styles.peek} aria-labelledby={titleId} data-task-id={task.id}>
      <header className={styles.peekHeader}>
        <div className={styles.peekHeading}>
          <span className={styles.peekRef}>{task.ref}</span>
          <h2 id={titleId} className={styles.peekTitle}>{task.title}</h2>
        </div>
        {onClose ? <button type="button" className={styles.peekClose} aria-label={closeLabel} onClick={onClose}><CloseIcon /></button> : null}
      </header>
      <div className={styles.peekBody}>
        <section className={styles.peekSection}>
          <h3 className={styles.peekLabel}>{statusLabel}</h3>
          <p className={styles.peekValue}><span className={`${styles.status} ${statusClass(task.status)}`}>{taskStatusLabel(task.status, locale)}</span></p>
        </section>
        {requiredStep ? (
          <section className={styles.peekSection}>
            <h3 className={styles.peekLabel}>{requiredStepLabel}</h3>
            <p className={styles.peekValue}>{requiredStep.title}</p>
          </section>
        ) : null}
        {task.currentRunId ? (
          <section className={styles.peekSection}>
            <h3 className={styles.peekLabel}>{runLabel}</h3>
            <code className={styles.peekRun}>{task.currentRunId}</code>
          </section>
        ) : null}
      </div>
      <footer className={styles.peekFooter}>
        {onOpenDetails ? <button type="button" className={styles.detailsButton} onClick={() => onOpenDetails(task.id)}>{detailsLabel}</button> : null}
      </footer>
    </aside>
  )
}

export function TaskWorkspaceExample({
  columns,
  selectedTaskId,
  sidePeek,
  onSelectTask,
  onClosePeek,
}: {
  readonly columns: readonly BoardColumnProps[]
  readonly selectedTaskId?: string | null
  readonly sidePeek?: TaskInspectorViewModel
  readonly onSelectTask?: (task: BoardTaskViewModel) => void
  readonly onClosePeek?: () => void
}) {
  return (
    <div className={styles.root}>
      <div className={sidePeek ? styles.workspaceWithPeek : styles.workspace}>
        <BoardColumns columns={columns} selectedTaskId={selectedTaskId} onSelectTask={onSelectTask} />
        {sidePeek ? <SidePeekFrame model={sidePeek} onClose={onClosePeek} /> : null}
      </div>
    </div>
  )
}
