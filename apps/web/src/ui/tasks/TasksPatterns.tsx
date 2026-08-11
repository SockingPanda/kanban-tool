import { useEffect, useId, useRef, useState, type ReactNode, type SyntheticEvent } from "react"

import { taskOpenerKey } from "../../lib/explorer-focus"
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
  TasksLocale,
  TasksListDisplay,
  TasksView,
  UnsupportedTasksView,
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

interface TaskMessages {
  readonly viewLabel: string
  readonly board: string
  readonly list: string
  readonly table: string
  readonly map: string
  readonly timeline: string
  readonly timelineUnavailable: string
  readonly searchLabel: string
  readonly searchPlaceholder: string
  readonly filterLabel: string
  readonly activeFilters: string
  readonly active: string
  readonly clearFilters: string
  readonly display: string
  readonly density: string
  readonly dense: string
  readonly comfortable: string
  readonly visibleFields: string
  readonly ref: string
  readonly boardLabel: string
  readonly taskCount: (count: number) => string
  readonly emptyColumn: string
  readonly emptyTable: string
  readonly tableCaption: string
  readonly loading: string
  readonly empty: string
  readonly offline: string
  readonly stale: string
  readonly recovering: string
  readonly error: string
  readonly loadingDetail: string
  readonly emptyDetail: string
  readonly offlineDetail: string
  readonly staleDetail: string
  readonly recoveringDetail: string
  readonly errorDetail: string
  readonly retry: string
  readonly status: string
  readonly requiredStep: string
  readonly run: string
  readonly closeDetails: string
  readonly openDetails: string
  readonly waitingDependency: string
  readonly steps: string
  readonly assigneeEmpty: string
}

const messages: Readonly<Record<TasksLocale, TaskMessages>> = {
  zh: {
    viewLabel: "任务视图",
    board: "看板",
    list: "列表",
    table: "表格",
    map: "关系图",
    timeline: "时间线",
    timelineUnavailable: "时间线需要 canonical timeline read model",
    searchLabel: "搜索",
    searchPlaceholder: "搜索标题或 ref",
    filterLabel: "筛选",
    activeFilters: "当前筛选",
    active: "已启用",
    clearFilters: "清除筛选",
    display: "显示",
    density: "密度",
    dense: "紧凑",
    comfortable: "舒适",
    visibleFields: "可见字段",
    ref: "引用",
    boardLabel: "任务看板",
    taskCount: (count) => `${count} 个任务`,
    emptyColumn: "暂无任务",
    emptyTable: "暂无任务",
    tableCaption: "任务表格",
    loading: "正在加载任务",
    empty: "暂无任务",
    offline: "当前离线",
    stale: "数据可能已过期",
    recovering: "正在恢复连接",
    error: "任务加载失败",
    loadingDetail: "正在读取 canonical board 数据。",
    emptyDetail: "这个 board 当前没有可展示的任务。",
    offlineDetail: "保留最近一次任务数据；联网后可以重试。",
    staleDetail: "实时连接中断，当前内容仍可查看。",
    recoveringDetail: "正在重新建立 board session。",
    errorDetail: "读取任务时发生错误，请重试。",
    retry: "重试",
    status: "状态",
    requiredStep: "必需步骤",
    run: "运行",
    closeDetails: "关闭任务详情",
    openDetails: "打开完整详情",
    waitingDependency: "等待依赖",
    steps: "步骤",
    assigneeEmpty: "—",
  },
  en: {
    viewLabel: "Task views",
    board: "Board",
    list: "List",
    table: "Table",
    map: "Map",
    timeline: "Timeline",
    timelineUnavailable: "Timeline needs a canonical timeline read model",
    searchLabel: "Search",
    searchPlaceholder: "Search title or ref",
    filterLabel: "Filters",
    activeFilters: "Active filters",
    active: "Active",
    clearFilters: "Clear filters",
    display: "Display",
    density: "Density",
    dense: "Dense",
    comfortable: "Comfortable",
    visibleFields: "Visible fields",
    ref: "Ref",
    boardLabel: "Task board",
    taskCount: (count) => `${count} tasks`,
    emptyColumn: "No tasks",
    emptyTable: "No tasks",
    tableCaption: "Task table",
    loading: "Loading tasks",
    empty: "No tasks",
    offline: "You are offline",
    stale: "Data may be stale",
    recovering: "Recovering connection",
    error: "Could not load tasks",
    loadingDetail: "Reading canonical board data.",
    emptyDetail: "This board has no tasks to display.",
    offlineDetail: "The latest task data is retained; retry when online.",
    staleDetail: "The live connection stopped; the current content is still readable.",
    recoveringDetail: "Re-establishing the board session.",
    errorDetail: "The task response could not be read. Try again.",
    retry: "Retry",
    status: "Status",
    requiredStep: "Required step",
    run: "Run",
    closeDetails: "Close task details",
    openDetails: "Open full details",
    waitingDependency: "Waiting on dependency",
    steps: "steps",
    assigneeEmpty: "—",
  },
}

function copyFor(locale: TasksLocale): TaskMessages {
  return messages[locale]
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

function stepsText(task: Pick<BoardTaskViewModel, "readiness">, locale: TasksLocale = "zh"): string {
  const { completedRequiredStepCount, requiredStepCount, optionalStepCount } = task.readiness
  const suffix = optionalStepCount > 0 ? ` + ${optionalStepCount}` : ""
  return `${completedRequiredStepCount}/${requiredStepCount}${suffix} ${copyFor(locale).steps}`
}

function taskStatusLabel(status: BoardTaskStatus, locale: TasksLocale): string {
  return (locale === "en" ? statusLabelsEnglish : statusLabels)[status] ?? status
}

function CloseIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="m3.5 3.5 9 9M12.5 3.5l-9 9" />
    </svg>
  )
}

interface ViewSwitcherCommonProps {
  readonly onViewChange?: (view: TasksView) => void
  readonly includeTableDisplay?: boolean
  readonly displayVariant?: TasksListDisplay
  readonly onDisplayChange?: (display: TasksListDisplay) => void
  readonly label?: string
  readonly locale?: TasksLocale
}

export type ViewSwitcherProps = ViewSwitcherCommonProps &
  (
    | { readonly activeView: TasksView; readonly includeUnsupportedTimeline?: false }
    | { readonly activeView: TasksView | UnsupportedTasksView; readonly includeUnsupportedTimeline: true }
  )

export function ViewSwitcher({ activeView, onViewChange, includeTableDisplay = false, displayVariant = "grouped", onDisplayChange, includeUnsupportedTimeline = false, label, locale = "zh" }: ViewSwitcherProps) {
  const copy = copyFor(locale)
  const views: readonly { readonly id: TasksView | UnsupportedTasksView | "table"; readonly label: string; readonly unsupported?: boolean }[] = [
    { id: "board", label: copy.board },
    { id: "list", label: copy.list },
    ...(includeTableDisplay ? [{ id: "table" as const, label: copy.table }] : []),
    { id: "map", label: copy.map },
    ...(includeUnsupportedTimeline ? [{ id: "timeline" as const, label: copy.timeline, unsupported: true }] : []),
  ]

  return (
    <div className={`${styles.root} ${styles.viewSwitcher}`} role="group" aria-label={label ?? copy.viewLabel}>
      {views.map((view) => {
        const unavailable = view.unsupported === true
        const isTableDisplay = view.id === "table"
        const listNeedsDisplayChange = view.id === "list" && includeTableDisplay && displayVariant === "table"
        const canChange = isTableDisplay ? Boolean(onDisplayChange && onViewChange) : Boolean(onViewChange) && (!listNeedsDisplayChange || Boolean(onDisplayChange))
        const inert = unavailable || !canChange
        const selected = isTableDisplay ? activeView === "list" && displayVariant === "table" : activeView === view.id && (!isTableDisplay && view.id === "list" ? displayVariant !== "table" : true)
        return (
          <button
            key={view.id}
            type="button"
            className={styles.viewButton}
            aria-pressed={selected}
            aria-disabled={inert ? true : undefined}
            disabled={inert}
            title={unavailable ? copy.timelineUnavailable : undefined}
            onClick={canChange && !unavailable ? () => {
              if (isTableDisplay) {
                onViewChange?.("list")
                onDisplayChange?.("table")
              } else {
                onViewChange?.(view.id as TasksView)
                if (view.id === "list") onDisplayChange?.("grouped")
              }
            } : undefined}
          >
            {view.label}
            {unavailable ? `（${locale === "en" ? "unavailable" : "不可用"}）` : ""}
          </button>
        )
      })}
    </div>
  )
}

export function ActiveFilter({ label, onRemove, removeLabel = "移除筛选" }: { readonly label: string; readonly onRemove?: () => void; readonly removeLabel?: string }) {
  return (
    <span className={`${styles.root} ${styles.chip}`}>
      <span>{label}</span>
      {onRemove ? (
        <button type="button" className={styles.chipRemove} aria-label={`${removeLabel}：${label}`} onClick={onRemove}>
          <CloseIcon />
        </button>
      ) : null}
    </span>
  )
}

export function FilterBar(props: TaskFilterBarProps) {
  const { placeholder, disabled = false, filters = [], onRemoveFilter, onClearFilters, onOpenFilters, filterButtonLabel, clearButtonLabel, locale = "zh" } = props
  const copy = copyFor(locale)
  const [uncontrolledSearch, setUncontrolledSearch] = useState(props.defaultSearch ?? "")
  const search = props.search ?? uncontrolledSearch
  const labelId = useId()
  const isControlled = props.search !== undefined

  function handleSearchChange(value: string) {
    if (isControlled) {
      props.onSearchChange(value)
    } else {
      setUncontrolledSearch(value)
    }
  }

  return (
    <div className={`${styles.root} ${styles.filterBar}`} role="search" aria-label={copy.searchLabel}>
      <label className={styles.searchField} htmlFor={labelId}>
        <span>{copy.searchLabel}</span>
        <input id={labelId} type="search" value={search} placeholder={placeholder ?? copy.searchPlaceholder} disabled={disabled} onChange={(event) => handleSearchChange(event.currentTarget.value)} />
      </label>
      <button type="button" className={styles.filterButton} disabled={disabled || !onOpenFilters} onClick={onOpenFilters}>
        {filterButtonLabel ?? copy.filterLabel}
      </button>
      {filters.length > 0 ? (
        <div className={styles.activeFilters} aria-label={copy.activeFilters}>
          <span className={styles.activeFiltersLabel}>{copy.active}</span>
          {filters.map((filter) => (
            <ActiveFilter
              key={filter.id}
              label={filter.label}
              removeLabel={locale === "en" ? "Remove filter" : "移除筛选"}
              onRemove={filter.removable === false ? undefined : onRemoveFilter ? () => onRemoveFilter(filter.id) : undefined}
            />
          ))}
          {onClearFilters ? <button type="button" className={styles.clearButton} onClick={onClearFilters}>{clearButtonLabel ?? copy.clearFilters}</button> : null}
        </div>
      ) : null}
    </div>
  )
}

export function DisplayMenu(props: DisplayMenuProps) {
  const { options, onDensityChange, onColumnVisibilityChange, columns = [], label, locale = "zh" } = props
  const copy = copyFor(locale)
  const controlled = props.open !== undefined
  const [uncontrolledOpen, setUncontrolledOpen] = useState(props.defaultOpen ?? false)
  const open = controlled ? props.open : uncontrolledOpen

  function handleToggle(event: SyntheticEvent<HTMLDetailsElement>) {
    const nextOpen = event.currentTarget.open
    if (!controlled) setUncontrolledOpen(nextOpen)
    props.onOpenChange?.(nextOpen)
  }

  return (
    <details className={`${styles.root} ${styles.displayMenu}`} open={open} onToggle={handleToggle}>
      <summary className={styles.displaySummary}>{label ?? copy.display}</summary>
      <div className={styles.displayPanel} role="group" aria-label={label ?? copy.display}>
        <p className={styles.displayHeading}>{copy.density}</p>
        <div className={styles.densityChoices}>
          {(["dense", "comfortable"] as const).map((density) => (
            <button key={density} type="button" className={styles.densityButton} aria-pressed={options.density === density} disabled={!onDensityChange} onClick={onDensityChange ? () => onDensityChange(density) : undefined}>
              {density === "dense" ? copy.dense : copy.comfortable}
            </button>
          ))}
        </div>
        {columns.length > 0 ? (
          <fieldset className={styles.columnToggles}>
            <legend className={styles.displayHeading}>{copy.visibleFields}</legend>
            {columns.map((column) => {
              const checked = options.visibleColumns?.[column.id] ?? true
              return (
                <label key={column.id} className={styles.columnToggle}>
                  <input type="checkbox" checked={checked} disabled={!onColumnVisibilityChange} onChange={onColumnVisibilityChange ? (event) => onColumnVisibilityChange(column.id, event.currentTarget.checked) : undefined} />
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
  const copy = copyFor(locale)
  const label = statusLabel ?? taskStatusLabel(task.status, locale)
  const className = `${styles.root} ${styles.card} ${densityClass(density)}`
  const content = (
    <>
      <span className={styles.cardMeta}>
        <span className={styles.cardRef} translate="no">{task.ref}</span>
        <span className={`${styles.status} ${statusClass(task.status)}`}>{label}</span>
      </span>
      <span className={styles.cardTitle}>{task.title}</span>
      <span className={styles.cardFooter}>
        {task.readiness.dependencyBlocked ? <span className={styles.dependency}>{copy.waitingDependency}</span> : <span />}
        <span className={styles.steps}>{stepsText(task, locale)}</span>
      </span>
    </>
  )

  if (!onSelect) {
    return <article className={`${className} ${styles.cardStatic}`}>{content}</article>
  }

  return (
    <button type="button" className={className} aria-pressed={selected} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelect(task)}>
      {content}
    </button>
  )
}

export function BoardColumn({ id, status, title, tasks, selectedTaskId = null, density = "dense", locale = "zh", onSelectTask, emptyLabel }: BoardColumnProps) {
  const copy = copyFor(locale)
  const headingId = `${useId()}-${id}-heading`
  return (
    <section className={`${styles.root} ${styles.column}`} aria-labelledby={headingId} data-status={status}>
      <header className={styles.columnHeader}>
        <h2 id={headingId} className={styles.columnTitle}>{title}</h2>
        <span className={styles.columnCount} aria-label={copy.taskCount(tasks.length)}>{tasks.length}</span>
      </header>
      <div className={styles.columnBody}>
        {tasks.length > 0 ? tasks.map((task) => (
          <TaskCard key={task.id} task={task} locale={locale} density={density} selected={selectedTaskId === task.id} onSelect={onSelectTask} />
        )) : <p className={styles.columnEmpty}>{emptyLabel ?? copy.emptyColumn}</p>}
      </div>
    </section>
  )
}

export function BoardColumns({ columns, density = "dense", selectedTaskId, locale = "zh", onSelectTask }: { readonly columns: readonly BoardColumnProps[]; readonly density?: TasksDensity; readonly selectedTaskId?: string | null; readonly locale?: TasksLocale; readonly onSelectTask?: (task: BoardTaskViewModel) => void }) {
  const copy = copyFor(locale)
  return (
    <div className={`${styles.root} ${styles.boardScroll}`}>
      <div className={styles.board} role="region" aria-label={copy.boardLabel}>
        {columns.map((column) => <BoardColumn key={column.id} {...column} locale={locale} density={density} selectedTaskId={selectedTaskId} onSelectTask={onSelectTask} />)}
      </div>
    </div>
  )
}

interface TaskTableColumn {
  readonly id: string
  readonly label: string
  readonly render: (task: BoardTaskViewModel) => ReactNode
}

export function TaskTable({ tasks, selectedTaskId = null, density = "dense", locale = "zh", visibleColumns, onSelectTask, caption }: TaskTableProps) {
  const copy = copyFor(locale)
  const tableClass = density === "comfortable" ? styles.tableComfortable : styles.tableDense
  const tableCaption = caption ?? copy.tableCaption
  const allColumns: readonly TaskTableColumn[] = [
    { id: "ref", label: copy.ref, render: (task) => <span className={styles.tableRef} translate="no">{task.ref}</span> },
    {
      id: "title",
      label: locale === "en" ? "Title" : "标题",
      render: (task) => onSelectTask ? <button type="button" className={styles.tableTask} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task)}>{task.title}</button> : <span className={styles.tableTaskStatic}>{task.title}</span>,
    },
    { id: "status", label: copy.status, render: (task) => <span className={`${styles.status} ${statusClass(task.status)}`}>{taskStatusLabel(task.status, locale)}</span> },
    { id: "priority", label: locale === "en" ? "Priority" : "优先级", render: (task) => <span translate="no">P{task.priority}</span> },
    { id: "steps", label: locale === "en" ? "Steps" : "步骤", render: (task) => <span className={styles.steps}>{stepsText(task, locale)}</span> },
    { id: "assignee", label: locale === "en" ? "Assignee" : "执行者", render: (task) => task.assignee ?? copy.assigneeEmpty },
  ]
  const columns = allColumns.filter((column) => visibleColumns?.[column.id] !== false)
  const visibleColumnsCount = Math.max(1, columns.length)

  return (
    <div className={`${styles.root} ${styles.tableWrap}`} role="region" aria-label={tableCaption} tabIndex={0} data-display-variant="table">
      <table className={`${styles.table} ${tableClass}`}>
        <caption className={styles.visuallyHidden}>{tableCaption}</caption>
        <thead><tr>{columns.map((column) => <th key={column.id} scope="col">{column.label}</th>)}</tr></thead>
        <tbody>
          {tasks.length > 0 ? tasks.map((task) => (
            <tr key={task.id} aria-selected={selectedTaskId === task.id}>{columns.map((column) => <td key={column.id}>{column.render(task)}</td>)}</tr>
          )) : (
            <tr><td colSpan={visibleColumnsCount} className={styles.tableEmpty}>{copy.emptyTable}</td></tr>
          )}
        </tbody>
      </table>
    </div>
  )
}

type StateCopyKey = "loading" | "empty" | "offline" | "stale" | "recovering" | "error"
type StateDetailCopyKey = "loadingDetail" | "emptyDetail" | "offlineDetail" | "staleDetail" | "recoveringDetail" | "errorDetail"

const stateDefaults: Readonly<Record<TaskStateBoundaryProps["state"], StateCopyKey>> = {
  loading: "loading",
  empty: "empty",
  offline: "offline",
  stale: "stale",
  recovering: "recovering",
  error: "error",
}

const stateDetailDefaults: Readonly<Record<TaskStateBoundaryProps["state"], StateDetailCopyKey>> = {
  loading: "loadingDetail",
  empty: "emptyDetail",
  offline: "offlineDetail",
  stale: "staleDetail",
  recovering: "recoveringDetail",
  error: "errorDetail",
}

export function TaskStateBoundary({ state, title, detail, actionLabel, onAction, children, locale = "zh" }: TaskStateBoundaryProps) {
  const copy = copyFor(locale)
  const role = state === "error" ? "alert" : "status"
  return (
    <section className={`${styles.root} ${styles.state}`} data-state={state} role={role} aria-live={state === "error" ? "assertive" : "polite"}>
      <h2 className={styles.stateTitle}>{title ?? copy[stateDefaults[state]]}</h2>
      <p className={styles.stateDetail}>{detail ?? copy[stateDetailDefaults[state]]}</p>
      {children}
      {onAction ? <button type="button" className={styles.stateAction} onClick={onAction}>{actionLabel ?? copy.retry}</button> : null}
    </section>
  )
}

export function SidePeekFrame({ model, locale = "zh", mode = "side-peek", onClose, onRestoreFocus, onOpenDetails, closeLabel, detailsLabel, statusLabel, requiredStepLabel, runLabel }: SidePeekFrameProps) {
  const copy = copyFor(locale)
  const task = model.task
  const titleId = useId()
  const peekRef = useRef<HTMLDivElement>(null)
  const requiredStep = model.steps.find((step) => step.required)

  useEffect(() => {
    if (mode !== "sheet") return
    const node = peekRef.current
    node?.focus()
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && onClose) {
        event.preventDefault()
        onClose()
        onRestoreFocus?.()
        return
      }
      if (event.key !== "Tab" || !node) return
      const focusable = Array.from(node.querySelectorAll<HTMLElement>("button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])"))
      if (focusable.length === 0) {
        event.preventDefault()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      const activeInside = active instanceof HTMLElement && node.contains(active)
      if (!activeInside || active === node) {
        event.preventDefault()
        const target = event.shiftKey ? last : first
        target.focus()
      } else if (event.shiftKey && active === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [mode, onClose, onRestoreFocus])

  function close() {
    onClose?.()
    onRestoreFocus?.()
  }

  const sheet = mode === "sheet"
  return (
    <div className={`${styles.root} ${styles.peekLayer}`} data-mode={mode}>
      {sheet && onClose ? <button type="button" className={styles.peekScrim} aria-label={closeLabel ?? copy.closeDetails} onClick={close} /> : null}
      <div ref={peekRef} className={styles.peek} role={sheet ? "dialog" : "complementary"} aria-modal={sheet ? true : undefined} aria-labelledby={titleId} data-task-id={task.id} data-mode={mode} tabIndex={sheet ? -1 : undefined}>
        <header className={styles.peekHeader}>
          <div className={styles.peekHeading}>
            <span className={styles.peekRef} translate="no">{task.ref}</span>
            <h2 id={titleId} className={styles.peekTitle}>{task.title}</h2>
          </div>
          {onClose ? <button type="button" className={styles.peekClose} aria-label={closeLabel ?? copy.closeDetails} onClick={close}><CloseIcon /></button> : null}
        </header>
        <div className={styles.peekBody}>
          <section className={styles.peekSection}>
            <h3 className={styles.peekLabel}>{statusLabel ?? copy.status}</h3>
            <p className={styles.peekValue}><span className={`${styles.status} ${statusClass(task.status)}`}>{taskStatusLabel(task.status, locale)}</span></p>
          </section>
          {requiredStep ? <section className={styles.peekSection}><h3 className={styles.peekLabel}>{requiredStepLabel ?? copy.requiredStep}</h3><p className={styles.peekValue}>{requiredStep.title}</p></section> : null}
          {task.currentRunId ? <section className={styles.peekSection}><h3 className={styles.peekLabel}>{runLabel ?? copy.run}</h3><code className={styles.peekRun} translate="no">{task.currentRunId}</code></section> : null}
        </div>
        <footer className={styles.peekFooter}>
          {onOpenDetails ? <button type="button" className={styles.detailsButton} onClick={() => onOpenDetails(task.id)}>{detailsLabel ?? copy.openDetails}</button> : null}
        </footer>
      </div>
    </div>
  )
}

export function TaskWorkspaceExample({ columns, selectedTaskId, sidePeek, locale = "zh", onSelectTask, onClosePeek }: { readonly columns: readonly BoardColumnProps[]; readonly selectedTaskId?: string | null; readonly sidePeek?: TaskInspectorViewModel; readonly locale?: TasksLocale; readonly onSelectTask?: (task: BoardTaskViewModel) => void; readonly onClosePeek?: () => void }) {
  return (
    <div className={styles.root}>
      <div className={sidePeek ? styles.workspaceWithPeek : styles.workspace}>
        <BoardColumns columns={columns} locale={locale} selectedTaskId={selectedTaskId} onSelectTask={onSelectTask} />
        {sidePeek ? <SidePeekFrame model={sidePeek} locale={locale} mode="side-peek" onClose={onClosePeek} /> : null}
      </div>
    </div>
  )
}
