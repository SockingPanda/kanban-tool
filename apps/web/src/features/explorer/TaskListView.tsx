import type { ExplorerReadError, TaskListPlanFilter, TaskListQueryState, TaskListSort, TaskListStatus } from "../../lib/api/explorer-read-model"
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

export function TaskListView({ state, rows, loading, error, onQueryChange, onSelectTask, onRetry }: TaskListViewProps) {
  const currentPage = Math.floor(state.meta.offset / Math.max(1, state.meta.limit)) + 1
  const totalPages = pageCount(state.meta)
  const canPrevious = currentPage > 1
  const canNext = currentPage < totalPages

  if (loading && rows.length === 0) {
    return <section className={styles.state} data-testid="task-list-loading" role="status">正在加载任务列表…</section>
  }

  return (
    <section className={styles.list} data-testid="task-list" aria-labelledby="task-list-heading">
      <header className={styles.toolbar}>
        <div className={styles.heading}>
          <p className={styles.eyebrow}>TASK EXPLORER</p>
          <h1 id="task-list-heading">任务列表</h1>
          <p className={styles.muted}>{loading ? "正在刷新…" : listRange(state.meta)}</p>
        </div>
        <label className={styles.searchField}>
          <span>搜索</span>
          <input
            data-testid="list-search"
            type="search"
            value={state.query.search}
            placeholder="标题、ref 或描述"
            onChange={(event) => updateQuery(state.query, onQueryChange, { search: event.currentTarget.value })}
          />
        </label>
      </header>

      <div className={styles.controls} aria-label="任务列表筛选">
        <label>
          <span>状态</span>
          <select
            data-testid="list-status-filter"
            value={state.query.status[0] ?? "all"}
            onChange={(event) => updateQuery(state.query, onQueryChange, { status: event.currentTarget.value === "all" ? [] : [event.currentTarget.value as TaskListStatus] })}
          >
            <option value="all">全部状态</option>
            {statuses.map((status) => <option key={status} value={status}>{status}</option>)}
          </select>
        </label>
        <label>
          <span>排序</span>
          <select data-testid="list-sort" value={state.query.sort} onChange={(event) => updateQuery(state.query, onQueryChange, { sort: event.currentTarget.value as TaskListSort })}>
            {sorts.map((sort) => <option key={sort} value={sort}>{sort}</option>)}
          </select>
        </label>
        <label>
          <span>每页</span>
          <select data-testid="list-limit" value={String(state.query.limit)} onChange={(event) => updateQuery(state.query, onQueryChange, { limit: Number(event.currentTarget.value) })}>
            {[25, 50, 100, 200].map((limit) => <option key={limit} value={limit}>{limit}</option>)}
          </select>
        </label>
        <label className={styles.archiveToggle}>
          <input type="checkbox" checked={state.query.includeArchived} onChange={(event) => updateQuery(state.query, onQueryChange, { includeArchived: event.currentTarget.checked })} />
          <span>包含 archived</span>
        </label>
        <fieldset>
          <legend>优先级</legend>
          {priorities.map((priority) => (
            <label key={priority} className={styles.inlineCheck}>
              <input type="checkbox" checked={state.query.priority.includes(priority)} onChange={() => updateQuery(state.query, onQueryChange, { priority: toggle(state.query.priority, priority) })} />
              <span>P{priority}</span>
            </label>
          ))}
        </fieldset>
        <fieldset>
          <legend>计划</legend>
          {plans.map((plan) => (
            <label key={plan} className={styles.inlineCheck}>
              <input type="checkbox" checked={state.query.plan.includes(plan)} onChange={() => updateQuery(state.query, onQueryChange, { plan: toggle(state.query.plan, plan) })} />
              <span>{plan}</span>
            </label>
          ))}
        </fieldset>
        <button type="button" className={styles.reset} onClick={() => onQueryChange({ status: [], priority: [], plan: [], search: "", sort: "updated_at", page: 1, limit: 100, includeArchived: false })}>重置</button>
      </div>

      {error ? (
        <div className={styles.error} role="alert" data-testid="task-list-error">
          <strong>任务列表加载失败</strong>
          <span>{error.message}</span>
          {onRetry ? <button type="button" onClick={onRetry}>重试</button> : null}
        </div>
      ) : null}

      {rows.length === 0 && !loading ? <p className={styles.empty} data-testid="task-list-empty" role="status">没有匹配的任务。</p> : null}
      {rows.length > 0 ? (
        <div className={styles.tableWrap} role="region" aria-label="任务列表内容" tabIndex={0}>
          <table className={styles.table}>
            <thead><tr><th>Ref</th><th>标题</th><th>状态</th><th>优先级</th><th>执行者</th><th>计划</th><th>步骤</th><th>更新</th></tr></thead>
            <tbody>
              {rows.map((task) => (
                <tr key={task.id} data-testid="task-row" data-task-id={task.id}>
                  <td className={styles.mono}>{task.ref}</td>
                  <td><button type="button" className={styles.taskLink} onClick={() => onSelectTask(task.id)}>{task.title}</button></td>
                  <td><span className={styles.badge}>{task.status}</span>{task.dependencyBlocked ? <span className={styles.muted}> blocked</span> : null}</td>
                  <td>P{task.priority}</td>
                  <td>{task.assignee || "—"}</td>
                  <td>{task.executionPlanState}</td>
                  <td>{task.completedRequiredStepCount} / {task.requiredStepCount}{task.optionalStepCount ? ` + ${task.optionalStepCount}` : ""}</td>
                  <td className={styles.mono}>{task.updatedAt}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      <footer className={styles.pagination}>
        <button type="button" disabled={!canPrevious} onClick={() => updateQuery(state.query, onQueryChange, { page: currentPage - 1 })}>上一页</button>
        <span>Page {currentPage} / {totalPages}</span>
        <button type="button" disabled={!canNext} onClick={() => updateQuery(state.query, onQueryChange, { page: currentPage + 1 })}>下一页</button>
      </footer>
    </section>
  )
}
