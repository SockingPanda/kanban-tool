import type { TaskListQueryState } from "../../lib/api/explorer-read-model"
import type { DensityMode, Locale } from "../../lib/preferences"
import { DisplayMenu, FilterBar, ViewSwitcher, type TasksDensity, type TasksListDisplay, type TasksView } from "../../ui/tasks"

import styles from "./TasksWorkspaceChrome.module.css"

export interface TasksDiagnosticLink {
  readonly id: string
  readonly label: string
  readonly href: string
}

export interface TasksWorkspaceChromeProps {
  readonly locale: Locale
  readonly scope: string
  readonly hrefForView?: (view: "board" | "list" | "map", display: TasksListDisplay) => string | undefined
  readonly activeView: TasksView
  readonly displayVariant: TasksListDisplay
  readonly density: DensityMode
  readonly listQuery?: TaskListQueryState
  readonly onViewChange: (view: TasksView, displayVariant: TasksListDisplay) => void
  readonly onSearchChange?: (search: string) => void
  readonly onOpenFilters?: () => void
  readonly onRemoveFilter?: (id: string) => void
  readonly onClearFilters?: () => void
  readonly onDensityChange: (density: DensityMode) => void
  readonly visibleColumns: Readonly<Record<string, boolean>>
  readonly onVisibleColumnsChange: (columnId: string, visible: boolean) => void
  readonly diagnostics: readonly TasksDiagnosticLink[]
  readonly hasInspector: boolean
  readonly onCloseInspector?: () => void
}

const displayColumns = {
  assignee: { id: "assignee", label: "执行者" },
  priority: { id: "priority", label: "优先级" },
  plan: { id: "plan", label: "计划" },
  steps: { id: "steps", label: "步骤" },
  updated: { id: "updated", label: "更新" },
} as const

const displayColumnsEnglish = {
  assignee: { id: "assignee", label: "Assignee" },
  priority: { id: "priority", label: "Priority" },
  plan: { id: "plan", label: "Plan" },
  steps: { id: "steps", label: "Steps" },
  updated: { id: "updated", label: "Updated" },
} as const

function taskDensity(density: DensityMode): TasksDensity {
  return density === "compact" ? "dense" : "comfortable"
}

function listFilters(query: TaskListQueryState | undefined, locale: Locale): readonly { readonly id: string; readonly label: string }[] {
  if (!query) return []
  const labels = locale === "en"
    ? { status: "Status", priority: "Priority", plan: "Plan", archived: "Archived" }
    : { status: "状态", priority: "优先级", plan: "计划", archived: "已归档" }
  return [
    ...(query.status.length > 0 ? [{ id: "status", label: `${labels.status}: ${query.status.join(", ")}` }] : []),
    ...(query.priority.length > 0 ? [{ id: "priority", label: `${labels.priority}: ${query.priority.map((value) => `P${value}`).join(", ")}` }] : []),
    ...(query.plan.length > 0 ? [{ id: "plan", label: `${labels.plan}: ${query.plan.join(", ")}` }] : []),
    ...(query.includeArchived ? [{ id: "include_archived", label: labels.archived }] : []),
  ]
}

export function TasksWorkspaceChrome({ locale, scope, hrefForView, activeView, displayVariant, density, listQuery, onViewChange, onSearchChange, onOpenFilters, onRemoveFilter, onClearFilters, onDensityChange, visibleColumns, onVisibleColumnsChange, diagnostics, hasInspector, onCloseInspector }: TasksWorkspaceChromeProps) {
  const copy = locale === "en"
    ? { title: "Tasks", views: "Board explorer views", search: "Search tasks", filters: "Filters", display: "Display", more: "More", diagnostics: "Diagnostics", close: "Close Inspector" }
    : { title: "任务", views: "看板浏览视图", search: "搜索任务", filters: "筛选", display: "显示", more: "更多", diagnostics: "诊断", close: "关闭任务检查器" }
  const columns = Object.values(locale === "en" ? displayColumnsEnglish : displayColumns)
  const filters = activeView === "list" ? listFilters(listQuery, locale) : []
  const querySearch = activeView === "list" ? listQuery?.search ?? "" : ""
  const searchDisabled = activeView !== "list" || onSearchChange === undefined
  const display = displayVariant === "table" ? "table" : "grouped"

  return (
    <section className={styles.chrome} aria-labelledby="tasks-workspace-heading" data-testid="tasks-workspace-chrome">
      <header className={styles.header}>
        <div className={styles.identity}>
          <h1 id="tasks-workspace-heading" tabIndex={-1} data-explorer-focus-fallback>{copy.title}</h1>
          <span className={styles.scope} translate="no">{scope}</span>
        </div>
        <div className={styles.actions}>
          <ViewSwitcher
            activeView={activeView}
            displayVariant={display}
            includeTableDisplay
            onSelectionChange={(view, nextDisplay) => onViewChange(view, nextDisplay)}
            hrefForView={hrefForView}
            label={copy.views}
            locale={locale}
          />
          <DisplayMenu
            options={{ density: taskDensity(density), visibleColumns }}
            onDensityChange={(next) => onDensityChange(next === "dense" ? "compact" : "comfortable")}
            columns={activeView === "list" ? columns : []}
            onColumnVisibilityChange={activeView === "list" ? onVisibleColumnsChange : undefined}
            label={copy.display}
            locale={locale}
          />
          {hasInspector && onCloseInspector ? <button type="button" className={styles.closeInspector} onClick={onCloseInspector}>{copy.close}</button> : null}
          {diagnostics.length > 0 ? (
            <details className={styles.more}>
              <summary>{copy.more}</summary>
              <nav aria-label={copy.diagnostics}>
                {diagnostics.map((link) => <a key={link.id} href={link.href}>{link.label}</a>)}
              </nav>
            </details>
          ) : null}
        </div>
      </header>
      <FilterBar
        search={querySearch}
        onSearchChange={onSearchChange ?? (() => undefined)}
        disabled={searchDisabled}
        placeholder={copy.search}
        filters={filters}
        onOpenFilters={activeView === "list" ? onOpenFilters : undefined}
        onRemoveFilter={onRemoveFilter}
        onClearFilters={onClearFilters}
        filterButtonLabel={copy.filters}
        locale={locale}
      />
    </section>
  )
}
