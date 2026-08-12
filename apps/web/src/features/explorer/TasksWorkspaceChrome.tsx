import type { MouseEvent } from "react"

import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"

import {
  CheckboxInput,
  PageFrame,
  SafeHStack,
  SafeVStack,
  Selector,
  TextInput,
} from "@/ui/astryx"

import type { TaskListQueryState } from "../../lib/api/explorer-read-model"
import type { DensityMode, Locale } from "../../lib/preferences"

export type TasksView = "board" | "list" | "map"
export type TasksListDisplay = "grouped" | "table"

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
  /** Intercept unmodified diagnostic clicks while retaining the anchor href fallback. */
  readonly onNavigate?: (target: string) => void | Promise<unknown>
  readonly hasInspector: boolean
  readonly onCloseInspector?: () => void
  readonly inert?: boolean
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

function taskDensity(density: DensityMode): "dense" | "comfortable" {
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

function handleDiagnosticClick(event: MouseEvent<HTMLAnchorElement>, href: string, onNavigate?: (target: string) => void | Promise<unknown>): void {
  if (!onNavigate || event.button !== 0 || event.defaultPrevented || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
  event.preventDefault()
  void onNavigate(href)
}

const viewOptions = [
  { id: "board", route: "board", display: "grouped", labels: { zh: "看板", en: "Board" } },
  { id: "list", route: "list", display: "grouped", labels: { zh: "列表", en: "List" } },
  { id: "table", route: "list", display: "table", labels: { zh: "表格", en: "Table" } },
  { id: "map", route: "map", display: "grouped", labels: { zh: "关系图", en: "Map" } },
] as const

function TaskViewNavigation({
  activeView,
  displayVariant,
  hrefForView,
  label,
  locale,
  onViewChange,
}: Pick<TasksWorkspaceChromeProps, "activeView" | "displayVariant" | "hrefForView" | "locale" | "onViewChange"> & { readonly label: string }) {
  function selection(view: (typeof viewOptions)[number], event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>, hasHref: boolean): void {
    const routeView = view.route as "board" | "list" | "map"
    if (hasHref) {
      const isModified = event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
      if (event.button !== 0 || isModified) return
      event.preventDefault()
    }
    onViewChange(routeView, view.display)
  }

  return (
    <SafeHStack as="div" role="group" aria-label={label} className="min-w-0 flex-wrap gap-1" data-testid="task-view-navigation">
      {viewOptions.map((view) => {
        const selected = view.id === "table"
          ? activeView === "list" && displayVariant === "table"
          : activeView === view.id && (view.id !== "list" || displayVariant !== "table")
        const href = hrefForView?.(view.route, view.display)
        const className = selected
          ? "rounded-md bg-accent-muted px-3 py-1.5 text-sm font-semibold text-accent no-underline outline-none focus-visible:outline-2 focus-visible:outline-accent"
          : "rounded-md px-3 py-1.5 text-sm text-primary no-underline outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent"
        return href !== undefined ? (
          <a
            key={view.id}
            className={className}
            href={href}
            aria-current={selected ? "page" : undefined}
            onClick={(event) => selection(view, event, true)}
          >
            {view.labels[locale]}
          </a>
        ) : (
          <button
            key={view.id}
            className={className}
            type="button"
            aria-pressed={selected}
            onClick={(event) => selection(view, event, false)}
          >
            {view.labels[locale]}
          </button>
        )
      })}
    </SafeHStack>
  )
}

function DisplayControls({
  columns,
  copy,
  density,
  onDensityChange,
  onVisibleColumnsChange,
  visibleColumns,
}: {
  readonly columns: readonly { readonly id: string; readonly label: string }[]
  readonly copy: {
    readonly display: string
    readonly density: string
    readonly dense: string
    readonly comfortable: string
    readonly visibleFields: string
    readonly select: string
    readonly loading: string
  }
  readonly density: DensityMode
  readonly onDensityChange: (density: DensityMode) => void
  readonly visibleColumns: Readonly<Record<string, boolean>>
  readonly onVisibleColumnsChange: (columnId: string, visible: boolean) => void
}) {
  const selectedDensity = taskDensity(density)
  const densityOptions = [
    { value: "dense", label: copy.dense },
    { value: "comfortable", label: copy.comfortable },
  ] as const

  return (
    <details className="relative min-w-0">
      <summary className="cursor-pointer rounded-md border border-border px-3 py-2 text-sm text-primary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent">
        {copy.display}
      </summary>
      <SafeVStack as="section" role="group" aria-label={copy.display} className="absolute end-0 z-10 mt-2 min-w-56 gap-3 rounded-lg border border-border bg-surface p-3 shadow-lg">
        <Selector
          label={copy.density}
          options={densityOptions}
          value={selectedDensity}
          placeholder={copy.select}
          loadingText={copy.loading}
          htmlName="tasks-density"
          onChange={(value) => {
            if (value === "dense" || value === "comfortable") onDensityChange(value === "dense" ? "compact" : "comfortable")
          }}
        />
        {columns.length > 0 ? (
          <SafeVStack as="fieldset" className="gap-2 border-0 p-0">
            <legend className="text-sm font-semibold text-primary">{copy.visibleFields}</legend>
            {columns.map((column) => (
              <CheckboxInput
                key={column.id}
                label={column.label}
                value={visibleColumns[column.id] ?? true}
                htmlName={`task-column-${column.id}`}
                onChange={(visible) => onVisibleColumnsChange(column.id, visible)}
                size="sm"
              />
            ))}
          </SafeVStack>
        ) : null}
      </SafeVStack>
    </details>
  )
}

export function TasksWorkspaceChrome({ locale, scope, hrefForView, activeView, displayVariant, density, listQuery, onViewChange, onSearchChange, onOpenFilters, onRemoveFilter, onClearFilters, onDensityChange, visibleColumns, onVisibleColumnsChange, diagnostics, onNavigate, hasInspector, onCloseInspector, inert = false }: TasksWorkspaceChromeProps) {
  const copy = locale === "en"
    ? {
        title: "Tasks", views: "Task views", search: "Search tasks", searchLabel: "Task search", filters: "Filters", display: "Display", more: "More", diagnostics: "Diagnostics", close: "Close Inspector", density: "Density", dense: "Compact", comfortable: "Comfortable", visibleFields: "Visible fields", select: "Select", loading: "Loading", active: "Active filters", clear: "Clear filters", removeFilter: "Remove filter",
      }
    : {
        title: "任务", views: "任务视图", search: "搜索任务", searchLabel: "任务搜索", filters: "筛选", display: "显示", more: "更多", diagnostics: "诊断", close: "关闭任务检查器", density: "密度", dense: "紧凑", comfortable: "舒适", visibleFields: "可见字段", select: "选择", loading: "加载中", active: "当前筛选", clear: "清除筛选", removeFilter: "移除筛选",
      }
  const columns = Object.values(locale === "en" ? displayColumnsEnglish : displayColumns)
  const filters = activeView === "list" ? listFilters(listQuery, locale) : []
  const querySearch = activeView === "list" ? listQuery?.search ?? "" : ""
  const searchDisabled = activeView !== "list" || onSearchChange === undefined

  const header = (
    <SafeHStack className="min-w-0 flex-wrap gap-3" align="center" justify="between">
      <SafeVStack className="min-w-0 gap-1">
        <Heading level={1} id="tasks-workspace-heading" tabIndex={-1} data-explorer-focus-fallback>{copy.title}</Heading>
        <Text type="supporting"><span translate="no">{scope}</span></Text>
      </SafeVStack>
      <SafeHStack className="min-w-0 flex-wrap gap-2" align="center" justify="end">
        <TaskViewNavigation activeView={activeView} displayVariant={displayVariant} hrefForView={hrefForView} label={copy.views} locale={locale} onViewChange={onViewChange} />
        <DisplayControls columns={activeView === "list" ? columns : []} copy={copy} density={density} onDensityChange={onDensityChange} visibleColumns={visibleColumns} onVisibleColumnsChange={onVisibleColumnsChange} />
        {hasInspector && onCloseInspector ? <Button label={copy.close} variant="ghost" size="sm" onClick={onCloseInspector} /> : null}
        {diagnostics.length > 0 ? (
          <details className="relative min-w-0">
            <summary className="cursor-pointer rounded-md border border-border px-3 py-2 text-sm text-primary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent">{copy.more}</summary>
            <SafeVStack as="nav" aria-label={copy.diagnostics} className="absolute end-0 z-10 mt-2 min-w-40 gap-1 rounded-lg border border-border bg-surface p-2 shadow-lg">
              {diagnostics.map((link) => <a key={link.id} className="rounded-md px-2 py-1.5 text-sm text-secondary no-underline hover:bg-overlay-hover hover:text-primary focus-visible:outline-2 focus-visible:outline-accent" href={link.href} onClick={onNavigate ? (event) => handleDiagnosticClick(event, link.href, onNavigate) : undefined}>{link.label}</a>)}
            </SafeVStack>
          </details>
        ) : null}
      </SafeHStack>
    </SafeHStack>
  )

  const toolbar = (
    <SafeHStack as="section" role="search" aria-label={copy.searchLabel} className="min-w-0 flex-wrap gap-3" align="end">
      <TextInput type="search" label={copy.search} value={querySearch} placeholder={copy.search} isDisabled={searchDisabled} isLabelHidden htmlName="task-search" data-testid="list-search" onChange={(value) => onSearchChange?.(value)} />
      <Button label={copy.filters} variant="secondary" size="sm" isDisabled={searchDisabled || onOpenFilters === undefined} onClick={onOpenFilters} />
      {filters.length > 0 ? (
        <SafeHStack as="section" aria-label={copy.active} className="min-w-0 flex-wrap gap-2" align="center">
          <Text type="supporting">{copy.active}</Text>
          {filters.map((filter) => (
            <Button key={filter.id} label={filter.label} variant="ghost" size="sm" aria-label={`${copy.removeFilter}: ${filter.label}`} onClick={onRemoveFilter ? () => onRemoveFilter(filter.id) : undefined} />
          ))}
          {onClearFilters ? <Button label={copy.clear} variant="ghost" size="sm" onClick={onClearFilters} /> : null}
        </SafeHStack>
      ) : null}
    </SafeHStack>
  )

  const frameHeader = (
    <SafeVStack className="min-w-0 gap-3">
      {header}
      {toolbar}
    </SafeVStack>
  )

  return (
    <SafeVStack as="section" className="min-w-0" inert={inert || undefined} data-testid="tasks-workspace-chrome">
      <PageFrame frame="content" aria-labelledby="tasks-workspace-heading" bodyLabel={copy.title} header={frameHeader}>
        <SafeVStack className="sr-only" aria-hidden="true" />
      </PageFrame>
    </SafeVStack>
  )
}
