import type { ExplorerReadError, TaskListPlanFilter, TaskListQueryState, TaskListSort, TaskListStatus } from "../../lib/api/explorer-read-model"
import { taskOpenerKey } from "../../lib/explorer-focus"
import type { Locale } from "../../lib/preferences"
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
  readonly locale?: Locale
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
  readonly error: string
  readonly offline: string
  readonly retry: string
  readonly empty: string
  readonly table: string
  readonly headers: readonly [string, string, string, string, string, string, string, string]
  readonly blocked: string
  readonly previous: string
  readonly page: string
  readonly pageSuffix: string
  readonly next: string
  readonly statusValues: Readonly<Record<TaskListStatus, string>>
  readonly planValues: Readonly<Record<TaskListPlanFilter, string>>
  readonly planState: Readonly<Record<"unplanned" | "planned" | "not_required", string>>
}

const copies: Record<Locale, ListCopy> = {
  zh: {
    eyebrow: "任务浏览", title: "任务列表", loading: "正在加载任务列表…", refreshing: "正在刷新…", search: "搜索", searchPlaceholder: "标题、ref 或描述", filters: "任务列表筛选", status: "状态", allStatuses: "全部状态", sort: "排序", pageSize: "每页", includeArchived: "包含已归档", priority: "优先级", plan: "计划", reset: "重置", error: "任务列表加载失败", offline: "当前离线，无法加载任务列表。", retry: "重试", empty: "没有匹配的任务。", table: "任务列表内容", headers: ["Ref", "标题", "状态", "优先级", "执行者", "计划", "步骤", "更新"], blocked: "阻塞", previous: "上一页", page: "第", pageSuffix: " 页", next: "下一页",
    statusValues: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    planValues: { plan_needed: "需要计划", has_steps: "有步骤", incomplete_required_steps: "必需步骤未完成" },
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
  },
  en: {
    eyebrow: "TASK EXPLORER", title: "Task list", loading: "Loading tasks…", refreshing: "Refreshing…", search: "Search", searchPlaceholder: "Title, ref, or description", filters: "Task list filters", status: "Status", allStatuses: "All statuses", sort: "Sort", pageSize: "Page size", includeArchived: "Include archived", priority: "Priority", plan: "Plan", reset: "Reset", error: "Task list failed to load", offline: "You are offline; the task list cannot be loaded.", retry: "Retry", empty: "No matching tasks.", table: "Task list", headers: ["Ref", "Title", "Status", "Priority", "Assignee", "Plan", "Steps", "Updated"], blocked: "blocked", previous: "Previous", page: "Page", pageSuffix: "", next: "Next",
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
  if (meta.total === 0) return "0 / 0"
  return `${meta.offset + 1}–${Math.min(meta.offset + meta.limit, meta.total)} / ${meta.total}`
}

export function TaskListView({ state, rows, loading, error, onQueryChange, onSelectTask, onRetry, locale = "zh" }: TaskListViewProps) {
  const copy = copies[locale]
  const currentPage = Math.floor(state.meta.offset / Math.max(1, state.meta.limit)) + 1
  const totalPages = pageCount(state.meta)
  const canPrevious = currentPage > 1
  const canNext = currentPage < totalPages

  if (loading && rows.length === 0) {
    return <section className={styles.state} data-testid="task-list-loading" role="status">{copy.loading}</section>
  }
  const offline = error instanceof Error && "kind" in error && error.kind === "offline"
  if (error && rows.length === 0) {
    return <section className={styles.state} data-testid={offline ? "task-list-offline" : "task-list-error"} role={offline ? "status" : "alert"}><h2>{offline ? copy.offline : copy.error}</h2>{!offline ? <p>{error.message}</p> : null}{onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}</section>
  }

  return (
    <section className={styles.list} data-testid="task-list" aria-labelledby="task-list-heading">
      <header className={styles.toolbar}>
        <div className={styles.heading}>
          <p className={styles.eyebrow}>{copy.eyebrow}</p>
          <h2 id="task-list-heading">{copy.title}</h2>
          <p className={styles.muted}>{loading ? copy.refreshing : listRange(state.meta)}</p>
        </div>
        <label className={styles.searchField}>
          <span>{copy.search}</span>
          <input
            data-testid="list-search"
            type="search"
            value={state.query.search}
            placeholder={copy.searchPlaceholder}
            onChange={(event) => updateQuery(state.query, onQueryChange, { search: event.currentTarget.value })}
          />
        </label>
      </header>

      <div className={styles.controls} aria-label={copy.filters}>
        <label>
          <span>{copy.status}</span>
          <select
            data-testid="list-status-filter"
            value={state.query.status[0] ?? "all"}
            onChange={(event) => updateQuery(state.query, onQueryChange, { status: event.currentTarget.value === "all" ? [] : [event.currentTarget.value as TaskListStatus] })}
          >
            <option value="all">{copy.allStatuses}</option>
            {statuses.map((status) => <option key={status} value={status}>{copy.statusValues[status]}</option>)}
          </select>
        </label>
        <label>
          <span>{copy.sort}</span>
          <select data-testid="list-sort" value={state.query.sort} onChange={(event) => updateQuery(state.query, onQueryChange, { sort: event.currentTarget.value as TaskListSort })}>
            {sorts.map((sort) => <option key={sort} value={sort}>{sort}</option>)}
          </select>
        </label>
        <label>
          <span>{copy.pageSize}</span>
          <select data-testid="list-limit" value={String(state.query.limit)} onChange={(event) => updateQuery(state.query, onQueryChange, { limit: Number(event.currentTarget.value) })}>
            {[25, 50, 100, 200].map((limit) => <option key={limit} value={limit}>{limit}</option>)}
          </select>
        </label>
        <label className={styles.archiveToggle}>
          <input type="checkbox" checked={state.query.includeArchived} onChange={(event) => updateQuery(state.query, onQueryChange, { includeArchived: event.currentTarget.checked })} />
          <span>{copy.includeArchived}</span>
        </label>
        <fieldset>
          <legend>{copy.priority}</legend>
          {priorities.map((priority) => (
            <label key={priority} className={styles.inlineCheck}>
              <input type="checkbox" checked={state.query.priority.includes(priority)} onChange={() => updateQuery(state.query, onQueryChange, { priority: toggle(state.query.priority, priority) })} />
              <span>P{priority}</span>
            </label>
          ))}
        </fieldset>
        <fieldset>
          <legend>{copy.plan}</legend>
          {plans.map((plan) => (
            <label key={plan} className={styles.inlineCheck}>
              <input type="checkbox" checked={state.query.plan.includes(plan)} onChange={() => updateQuery(state.query, onQueryChange, { plan: toggle(state.query.plan, plan) })} />
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

      {rows.length === 0 && !loading ? <p className={styles.empty} data-testid="task-list-empty" role="status">{copy.empty}</p> : null}
      {rows.length > 0 ? (
        <div className={styles.tableWrap} role="region" aria-label={copy.table} tabIndex={0}>
          <table className={styles.table}>
            <caption className={styles.visuallyHidden}>{copy.table}</caption>
            <thead><tr>{copy.headers.map((header) => <th key={header} scope="col">{header}</th>)}</tr></thead>
            <tbody>
              {rows.map((task) => (
                <tr key={task.id} data-testid="task-row" data-task-id={task.id}>
                  <td className={styles.mono}>{task.ref}</td>
                  <td><button type="button" className={styles.taskLink} data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>{task.title}</button></td>
                  <td><span className={styles.badge}>{copy.statusValues[task.status]}</span>{task.dependencyBlocked ? <span className={styles.muted}> {copy.blocked}</span> : null}</td>
                  <td>P{task.priority}</td>
                  <td>{task.assignee || "—"}</td>
                  <td>{copy.planState[task.executionPlanState]}</td>
                  <td>{task.completedRequiredStepCount} / {task.requiredStepCount}{task.optionalStepCount ? ` + ${task.optionalStepCount}` : ""}</td>
                  <td className={styles.mono}>{task.updatedAt}</td>
                </tr>
              ))}
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
