import type { ReactNode } from "react"

import type { ExplorerReadError, TaskListPlanFilter, TaskListQueryState, TaskListSort, TaskListStatus } from "../../lib/api/explorer-read-model"
import { taskOpenerKey } from "../../lib/explorer-focus"
import type { Locale } from "../../lib/preferences"
import type { TasksDensity } from "../../ui/tasks"
import { activeAttentionLens, attentionCounts, attentionLenses, queryWithAttentionLens, type AttentionLens } from "../attention/attention-lens"
import styles from "./TaskListView.module.css"

export interface TaskListRow {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: TaskListStatus
  readonly priority: number
  readonly assignee: string | null
  readonly executionPlanState: "unplanned" | "planned" | "not_required"
  readonly dependencyBlocked: boolean
  readonly requiredStepCount: number
  readonly completedRequiredStepCount: number
  readonly optionalStepCount: number
  readonly updatedAt: number
}

export interface TaskListViewState {
  readonly query: TaskListQueryState
  readonly meta: { readonly offset: number; readonly limit: number; readonly total: number }
}

export interface TaskListViewProps {
  readonly state: TaskListViewState
  readonly rows: readonly TaskListRow[]
  readonly loading: boolean
  readonly error?: ExplorerReadError | Error | null
  readonly onQueryChange: (query: TaskListQueryState) => void
  readonly onSelectTask: (taskId: string) => void
  readonly onRetry?: () => void
  readonly onCreate?: (trigger?: HTMLElement | null) => void
  readonly isMutationPending?: boolean
  readonly locale?: Locale
  /** The List and Table projections share the same canonical page read. */
  readonly displayVariant?: "grouped" | "table"
  /** View-local presentation choices; never serialized as canonical task state. */
  readonly visibleColumns?: Readonly<Record<string, boolean>>
  readonly showToolbarSearch?: boolean
  /** Keep the section heading in the accessibility tree while allowing the workspace chrome to own the visible title. */
  readonly showHeading?: boolean
  /** User-selected list geometry; this must affect both grouped and table projections. */
  readonly density?: TasksDensity
}

type ListCopy = {
  readonly eyebrow: string
  readonly title: string
  readonly loading: string
  readonly refreshing: string
  readonly search: string
  readonly searchPlaceholder: string
  readonly filters: string
  readonly status: string
  readonly allStatuses: string
  readonly sort: string
  readonly pageSize: string
  readonly includeArchived: string
  readonly priority: string
  readonly plan: string
  readonly reset: string
  readonly attentionLens: string
  readonly attentionScope: string
  readonly attentionLabels: Readonly<Record<AttentionLens, string>>
  readonly activeAttention: (label: string) => string
  readonly clearAttention: string
  readonly error: string
  readonly offline: string
  readonly retry: string
  readonly emptyBoard: string
  readonly emptyPage: string
  readonly noMatches: string
  readonly table: string
  readonly headers: readonly [string, string, string, string, string, string, string, string]
  readonly blocked: string
  readonly previous: string
  readonly page: string
  readonly pageSuffix: string
  readonly next: string
  readonly createTask: string
  readonly statusValues: Readonly<Record<TaskListStatus, string>>
  readonly planValues: Readonly<Record<TaskListPlanFilter, string>>
  readonly planState: Readonly<Record<"unplanned" | "planned" | "not_required", string>>
}

const copies: Record<Locale, ListCopy> = {
  zh: {
    eyebrow: "任务浏览", title: "任务列表", loading: "正在加载任务列表…", refreshing: "正在刷新…", search: "搜索", searchPlaceholder: "标题、ref 或描述", filters: "任务列表筛选", status: "状态", allStatuses: "全部状态", sort: "排序", pageSize: "每页", includeArchived: "包含已归档", priority: "优先级", plan: "计划", reset: "重置", attentionLens: "关注入口", attentionScope: "计数基于当前已加载结果。", attentionLabels: { ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核" }, activeAttention: (label) => `当前关注：${label}`, clearAttention: "清除关注筛选", error: "任务列表加载失败", offline: "当前离线，无法加载任务列表。", retry: "重试", emptyBoard: "这个看板还没有任务。", emptyPage: "当前页没有任务，请返回上一页。", noMatches: "没有任务符合当前筛选。", table: "任务列表内容", headers: ["Ref", "标题", "状态", "优先级", "执行者", "计划", "步骤", "更新"], blocked: "阻塞", previous: "上一页", page: "第", pageSuffix: " 页", next: "下一页", createTask: "创建任务",
    statusValues: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    planValues: { plan_needed: "需要计划", has_steps: "有步骤", incomplete_required_steps: "必需步骤未完成" },
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
  },
  en: {
    eyebrow: "TASK EXPLORER", title: "Task list", loading: "Loading tasks…", refreshing: "Refreshing…", search: "Search", searchPlaceholder: "Title, ref, or description", filters: "Task list filters", status: "Status", allStatuses: "All statuses", sort: "Sort", pageSize: "Page size", includeArchived: "Include archived", priority: "Priority", plan: "Plan", reset: "Reset", attentionLens: "Attention", attentionScope: "Counts reflect the currently loaded results.", attentionLabels: { ready: "Ready", running: "Running", blocked: "Blocked", review: "Review" }, activeAttention: (label) => `Attention: ${label}`, clearAttention: "Clear attention filter", error: "Task list failed to load", offline: "You are offline; the task list cannot be loaded.", retry: "Retry", emptyBoard: "This board has no tasks yet.", emptyPage: "There are no tasks on this page. Go back to the previous page.", noMatches: "No tasks match the current filters.", table: "Task list", headers: ["Ref", "Title", "Status", "Priority", "Assignee", "Plan", "Steps", "Updated"], blocked: "blocked", previous: "Previous", page: "Page", pageSuffix: "", next: "Next", createTask: "Create task",
    statusValues: { triage: "Triage", todo: "To do", scheduled: "Scheduled", ready: "Ready", running: "Running", blocked: "Blocked", review: "Review", done: "Done", archived: "Archived" },
    planValues: { plan_needed: "Plan needed", has_steps: "Has steps", incomplete_required_steps: "Incomplete required steps" },
    planState: { unplanned: "Unplanned", planned: "Planned", not_required: "Not required" },
  },
}

const statuses: readonly TaskListStatus[] = ["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"]
const priorities = [0, 1, 2, 3] as const
const plans: readonly TaskListPlanFilter[] = ["plan_needed", "has_steps", "incomplete_required_steps"]
const sorts: readonly TaskListSort[] = [
  "updated_at",
  "-updated_at",
  "seq",
  "-seq",
  "title",
  "-title",
  "status",
  "-status",
  "priority",
  "-priority",
  "assignee",
  "-assignee",
  "scheduled_at",
  "-scheduled_at",
  "due_at",
  "-due_at",
  "created_at",
  "-created_at",
  "position",
  "-position",
]

function updateQuery(
  query: TaskListQueryState,
  onQueryChange: TaskListViewProps["onQueryChange"],
  patch: Partial<TaskListQueryState>,
): void {
  onQueryChange({ ...query, ...patch, page: patch.page ?? 1 })
}

function toggle<T>(values: readonly T[], value: T): T[] {
  return values.includes(value) ? values.filter((candidate) => candidate !== value) : [...values, value]
}

function pageCount(meta: TaskListViewState["meta"]): number {
  return Math.max(1, Math.ceil(meta.total / Math.max(1, meta.limit)))
}

function listRange(meta: TaskListViewState["meta"]): string {
  if (meta.total === 0 || meta.offset >= meta.total) return `0 / ${meta.total}`
  return `${meta.offset + 1}–${Math.min(meta.offset + meta.limit, meta.total)} / ${meta.total}`
}

export function TaskListView({ state, rows, loading, error, onQueryChange, onSelectTask, onRetry, onCreate, isMutationPending = false, locale = "zh", displayVariant = "table", visibleColumns, showToolbarSearch = true, showHeading = true, density = "comfortable" }: TaskListViewProps) {
  const copy = copies[locale]
  const attentionCountsByStatus = attentionCounts(rows)
  const selectedAttention = activeAttentionLens(state.query.status)
  const hasTaskFilters = state.query.status.length > 0
    || state.query.priority.length > 0
    || state.query.plan.length > 0
    || state.query.search.trim().length > 0
    || state.query.includeArchived
  const currentPage = Math.floor(state.meta.offset / Math.max(1, state.meta.limit)) + 1
  const totalPages = pageCount(state.meta)
  const canPrevious = currentPage > 1
  const canNext = currentPage < totalPages
  const emptyKind = hasTaskFilters ? "filter" : state.meta.total > 0 ? "page" : "board"

  if (loading && rows.length === 0) {
    return <section className={styles.state} data-testid="task-list-loading" role="status">{copy.loading}</section>
  }
  const offline = error instanceof Error && "kind" in error && error.kind === "offline"
  if (error && rows.length === 0) {
    return <section className={styles.state} data-testid={offline ? "task-list-offline" : "task-list-error"} role={offline ? "status" : "alert"}><h2>{offline ? copy.offline : copy.error}</h2>{!offline ? <p>{error.message}</p> : null}{onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}</section>
  }

  return (
    <section className={styles.list} data-testid="task-list" data-density={density} aria-labelledby="task-list-heading">
      <header className={styles.toolbar}>
        <div className={styles.heading}>
          {showHeading ? <p className={styles.eyebrow}>{copy.eyebrow}</p> : null}
          <h2 id="task-list-heading" className={showHeading ? undefined : styles.visuallyHidden}>{copy.title}</h2>
          <p className={styles.muted}>{loading ? copy.refreshing : listRange(state.meta)}</p>
        </div>
        <div className={styles.toolbarActions}>
          {showToolbarSearch ? <label className={styles.searchField}>
            <span>{copy.search}</span>
            <input
              data-testid="list-search"
              name="task-search"
              type="search"
              value={state.query.search}
              placeholder={copy.searchPlaceholder}
              onChange={(event) => updateQuery(state.query, onQueryChange, { search: event.currentTarget.value })}
            />
          </label> : null}
          {onCreate ? <button type="button" className={styles.createButton} disabled={isMutationPending} onClick={(event) => onCreate(event.currentTarget)} data-testid="task-create">{copy.createTask}</button> : null}
        </div>
      </header>

      <div className={styles.attentionBar} role="group" aria-label={copy.attentionLens} data-testid="task-attention-lens" data-count-scope="loaded-results">
        <span className={styles.attentionLabel}>{copy.attentionLens}</span>
        <span className={styles.attentionScope}>{copy.attentionScope}</span>
        {attentionLenses.map((lens) => (
          <button
            key={lens}
            type="button"
            className={`${styles.attentionChip} ${selectedAttention === lens ? styles.attentionChipActive : ""}`}
            aria-pressed={selectedAttention === lens}
            data-testid={`attention-lens-${lens}`}
            onClick={() => onQueryChange(queryWithAttentionLens(state.query, lens))}
          >
            <span>{copy.attentionLabels[lens]}</span>
            <span className={styles.attentionCount} data-testid={`attention-count-${lens}`}>{attentionCountsByStatus[lens]}</span>
          </button>
        ))}
        {selectedAttention !== null ? (
          <div className={styles.activeFilter} data-testid="attention-active-filter">
            <span>{copy.activeAttention(copy.attentionLabels[selectedAttention])}</span>
            <button type="button" onClick={() => updateQuery(state.query, onQueryChange, { status: [] })} data-testid="attention-clear">{copy.clearAttention}</button>
          </div>
        ) : null}
      </div>

      <div id="task-list-controls" className={styles.controls} role="group" aria-label={copy.filters} tabIndex={-1}>
        <label>
          <span>{copy.status}</span>
          <select
            data-testid="list-status-filter"
            name="task-status"
            value={state.query.status[0] ?? "all"}
            onChange={(event) => updateQuery(state.query, onQueryChange, { status: event.currentTarget.value === "all" ? [] : [event.currentTarget.value as TaskListStatus] })}
          >
            <option value="all">{copy.allStatuses}</option>
            {statuses.map((status) => <option key={status} value={status}>{copy.statusValues[status]}</option>)}
          </select>
        </label>
        <label>
          <span>{copy.sort}</span>
          <select data-testid="list-sort" name="task-sort" value={state.query.sort} onChange={(event) => updateQuery(state.query, onQueryChange, { sort: event.currentTarget.value as TaskListSort })}>
            {sorts.map((sort) => <option key={sort} value={sort}>{sort}</option>)}
          </select>
        </label>
        <label>
          <span>{copy.pageSize}</span>
          <select data-testid="list-limit" name="task-limit" value={String(state.query.limit)} onChange={(event) => updateQuery(state.query, onQueryChange, { limit: Number(event.currentTarget.value) })}>
            {[25, 50, 100, 200].map((limit) => <option key={limit} value={limit}>{limit}</option>)}
          </select>
        </label>
        <label className={styles.archiveToggle}>
          <input type="checkbox" name="task-include-archived" checked={state.query.includeArchived} onChange={(event) => updateQuery(state.query, onQueryChange, { includeArchived: event.currentTarget.checked })} />
          <span>{copy.includeArchived}</span>
        </label>
        <fieldset>
          <legend>{copy.priority}</legend>
          {priorities.map((priority) => (
            <label key={priority} className={styles.inlineCheck}>
              <input type="checkbox" name="task-priority" value={priority} checked={state.query.priority.includes(priority)} onChange={() => updateQuery(state.query, onQueryChange, { priority: toggle(state.query.priority, priority) })} />
              <span>P{priority}</span>
            </label>
          ))}
        </fieldset>
        <fieldset>
          <legend>{copy.plan}</legend>
          {plans.map((plan) => (
            <label key={plan} className={styles.inlineCheck}>
              <input type="checkbox" name="task-plan" value={plan} checked={state.query.plan.includes(plan)} onChange={() => updateQuery(state.query, onQueryChange, { plan: toggle(state.query.plan, plan) })} />
              <span>{copy.planValues[plan]}</span>
            </label>
          ))}
        </fieldset>
        <button type="button" className={styles.reset} onClick={() => onQueryChange({ status: [], priority: [], plan: [], search: "", sort: "updated_at", page: 1, limit: 100, includeArchived: false })}>{copy.reset}</button>
      </div>

      {error ? (
        <div className={styles.error} role={offline ? "status" : "alert"} data-testid={offline ? "task-list-offline" : "task-list-error"}>
          <strong>{offline ? copy.offline : copy.error}</strong>
          {!offline ? <span>{error.message}</span> : null}
          {onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}
        </div>
      ) : null}

      {rows.length === 0 && !loading ? (
        <div className={styles.empty} data-testid="task-list-empty" data-empty-kind={emptyKind} role="status">
          <p data-testid={`task-list-${emptyKind}-empty`}>
            {emptyKind === "filter" ? copy.noMatches : emptyKind === "page" ? copy.emptyPage : copy.emptyBoard}
          </p>
        </div>
      ) : null}
      {rows.length > 0 ? displayVariant === "grouped" ? (
        <ul className={styles.groupedList} data-display-variant="list" aria-label={copy.table}>
          {statuses.map((status) => {
            const group = rows.filter((task) => task.status === status)
            if (group.length === 0) return null
            return (
              <li className={styles.statusGroup} key={status} aria-labelledby={`task-status-${status}`}>
                <header className={styles.statusGroupHeader}>
                  <h3 id={`task-status-${status}`}>{copy.statusValues[status]}</h3>
                  <span>{group.length}</span>
                </header>
                <ul className={styles.statusGroupRows} aria-labelledby={`task-status-${status}`}>
                  {group.map((task) => (
                    <li className={styles.groupedRow} key={task.id} data-testid="task-row" data-task-id={task.id}>
                      <div className={styles.groupedIdentity}>
                        <span className={styles.mono}>{task.ref}</span>
                        <button type="button" className={styles.taskLink} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>{task.title}</button>
                      </div>
                      <dl className={styles.groupedFacts}>
                        {visibleColumns?.priority !== false ? <div><dt className={styles.visuallyHidden}>{copy.headers[3]}</dt><dd>P{task.priority}</dd></div> : null}
                        {visibleColumns?.assignee !== false ? <div><dt className={styles.visuallyHidden}>{copy.headers[4]}</dt><dd>{task.assignee || "—"}</dd></div> : null}
                        {visibleColumns?.plan !== false ? <div><dt className={styles.visuallyHidden}>{copy.headers[5]}</dt><dd>{copy.planState[task.executionPlanState]}</dd></div> : null}
                        {visibleColumns?.steps !== false ? <div><dt className={styles.visuallyHidden}>{copy.headers[6]}</dt><dd>{task.completedRequiredStepCount} / {task.requiredStepCount}{task.optionalStepCount ? ` + ${task.optionalStepCount}` : ""}</dd></div> : null}
                        {visibleColumns?.updated !== false ? <div><dt className={styles.visuallyHidden}>{copy.headers[7]}</dt><dd className={styles.mono}>{task.updatedAt}</dd></div> : null}
                          {task.dependencyBlocked ? <div><dt className={styles.visuallyHidden}>{copy.statusValues.blocked}</dt><dd className={styles.muted}>{copy.blocked}</dd></div> : null}
                      </dl>
                    </li>
                  ))}
                </ul>
              </li>
            )
          })}
        </ul>
      ) : (
        <div className={styles.tableWrap} role="region" aria-label={copy.table} tabIndex={0} data-display-variant="table">
          <table className={styles.table}>
            <caption className={styles.visuallyHidden}>{copy.table}</caption>
            <thead><tr>{[
              ["ref", copy.headers[0]],
              ["title", copy.headers[1]],
              ["status", copy.headers[2]],
              ["priority", copy.headers[3]],
              ["assignee", copy.headers[4]],
              ["plan", copy.headers[5]],
              ["steps", copy.headers[6]],
              ["updated", copy.headers[7]],
            ].filter(([id]) => id === "ref" || id === "title" || id === "status" || visibleColumns?.[id] !== false).map(([id, header]) => <th key={id} scope="col">{header}</th>)}</tr></thead>
            <tbody>
              {rows.map((task) => {
                const cells: readonly [string, ReactNode][] = [
                  ["ref", <span className={styles.mono} key="ref">{task.ref}</span>],
                  ["title", <button type="button" className={styles.taskLink} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)} key="title">{task.title}</button>],
                  ["status", <span key="status"><span className={styles.badge}>{copy.statusValues[task.status]}</span>{task.dependencyBlocked ? <span className={styles.muted}> {copy.blocked}</span> : null}</span>],
                  ["priority", <span key="priority">P{task.priority}</span>],
                  ["assignee", <span key="assignee">{task.assignee || "—"}</span>],
                  ["plan", <span key="plan">{copy.planState[task.executionPlanState]}</span>],
                  ["steps", <span key="steps">{task.completedRequiredStepCount} / {task.requiredStepCount}{task.optionalStepCount ? ` + ${task.optionalStepCount}` : ""}</span>],
                  ["updated", <span className={styles.mono} key="updated">{task.updatedAt}</span>],
                ]
                return <tr key={task.id} data-testid="task-row" data-task-id={task.id}>{cells.filter(([id]) => id === "ref" || id === "title" || id === "status" || visibleColumns?.[id] !== false).map(([id, cell]) => <td key={id}>{cell}</td>)}</tr>
              })}
            </tbody>
          </table>
        </div>
      ) : null}

      <footer className={styles.pagination}>
        <button type="button" disabled={!canPrevious} onClick={() => updateQuery(state.query, onQueryChange, { page: currentPage - 1 })}>{copy.previous}</button>
        <span>{copy.page} {currentPage}{copy.pageSuffix} / {totalPages}</span>
        <button type="button" disabled={!canNext} onClick={() => updateQuery(state.query, onQueryChange, { page: currentPage + 1 })}>{copy.next}</button>
      </footer>
    </section>
  )
}
