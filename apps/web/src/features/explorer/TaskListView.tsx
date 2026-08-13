import type { ReactNode } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { EmptyState } from "@astryxdesign/core/EmptyState"
import { Heading } from "@astryxdesign/core/Heading"
import { List, ListItem } from "@astryxdesign/core/List"
import { Table, TableBody, TableCell, TableHeader, TableHeaderCell, TableRow } from "@astryxdesign/core/Table"
import { Text } from "@astryxdesign/core/Text"

import type { ExplorerReadError, TaskListPlanFilter, TaskListQueryState, TaskListSort, TaskListStatus } from "../../lib/api/explorer-read-model"
import { taskOpenerKey } from "../../lib/explorer-focus"
import type { Locale } from "../../lib/preferences"
import { activeAttentionLens, attentionCounts, attentionLenses, queryWithAttentionLens, type AttentionLens } from "../attention/attention-lens"
import { CheckboxInput, MultiSelector, SafeHStack, SafeSection, SafeVStack, Selector, TextInput } from "@/ui/astryx"

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

type TasksDensity = "dense" | "comfortable"

type TaskTableColumn = "ref" | "title" | "status" | "priority" | "assignee" | "plan" | "steps" | "updated"

const tableColumnClasses: Readonly<Record<TaskTableColumn, string>> = {
  ref: "w-32 min-w-32",
  title: "w-64 min-w-64",
  status: "w-36 min-w-36",
  priority: "w-24 min-w-24",
  assignee: "w-40 min-w-40",
  plan: "w-40 min-w-40",
  steps: "w-32 min-w-32",
  updated: "w-36 min-w-36",
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
  readonly allPriorities: string
  readonly plan: string
  readonly allPlans: string
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
    eyebrow: "任务浏览", title: "任务列表", loading: "正在加载任务列表…", refreshing: "正在刷新…", search: "搜索", searchPlaceholder: "标题、ref 或描述", filters: "任务列表筛选", status: "状态", allStatuses: "全部状态", sort: "排序", pageSize: "每页", includeArchived: "包含已归档", priority: "优先级", allPriorities: "全部优先级", plan: "计划", allPlans: "全部计划", reset: "重置", attentionLens: "关注入口", attentionScope: "计数基于当前已加载结果。", attentionLabels: { ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核" }, activeAttention: (label) => `当前关注：${label}`, clearAttention: "清除关注筛选", error: "任务列表加载失败", offline: "当前离线，无法加载任务列表。", retry: "重试", emptyBoard: "这个看板还没有任务。", emptyPage: "当前页没有任务，请返回上一页。", noMatches: "没有任务符合当前筛选。", table: "任务列表内容", headers: ["Ref", "标题", "状态", "优先级", "执行者", "计划", "步骤", "更新"], blocked: "阻塞", previous: "上一页", page: "第", pageSuffix: " 页", next: "下一页", createTask: "创建任务",
    statusValues: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    planValues: { plan_needed: "需要计划", has_steps: "有步骤", incomplete_required_steps: "必需步骤未完成" },
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
  },
  en: {
    eyebrow: "TASK EXPLORER", title: "Task list", loading: "Loading tasks…", refreshing: "Refreshing…", search: "Search", searchPlaceholder: "Title, ref, or description", filters: "Task list filters", status: "Status", allStatuses: "All statuses", sort: "Sort", pageSize: "Page size", includeArchived: "Include archived", priority: "Priority", allPriorities: "All priorities", plan: "Plan", allPlans: "All plan states", reset: "Reset", attentionLens: "Attention", attentionScope: "Counts reflect the currently loaded results.", attentionLabels: { ready: "Ready", running: "Running", blocked: "Blocked", review: "Review" }, activeAttention: (label) => `Attention: ${label}`, clearAttention: "Clear attention filter", error: "Task list failed to load", offline: "You are offline; the task list cannot be loaded.", retry: "Retry", emptyBoard: "This board has no tasks yet.", emptyPage: "There are no tasks on this page. Go back to the previous page.", noMatches: "No tasks match the current filters.", table: "Task list", headers: ["Ref", "Title", "Status", "Priority", "Assignee", "Plan", "Steps", "Updated"], blocked: "blocked", previous: "Previous", page: "Page", pageSuffix: "", next: "Next", createTask: "Create task",
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

function pageCount(meta: TaskListViewState["meta"]): number {
  return Math.max(1, Math.ceil(meta.total / Math.max(1, meta.limit)))
}

function listRange(meta: TaskListViewState["meta"]): string {
  if (meta.total === 0 || meta.offset >= meta.total) return `0 / ${meta.total}`
  return `${meta.offset + 1}–${Math.min(meta.offset + meta.limit, meta.total)} / ${meta.total}`
}

function tableDensity(density: TasksDensity): "compact" | "balanced" {
  return density === "dense" ? "compact" : "balanced"
}

function statusBadgeVariant(status: TaskListStatus): "neutral" | "info" | "success" | "warning" | "error" {
  if (status === "done") return "success"
  if (status === "blocked") return "error"
  if (status === "review") return "warning"
  if (status === "ready" || status === "running") return "info"
  return "neutral"
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
    return (
      <SafeSection variant="transparent" padding={0} className="min-h-48" data-testid="task-list-loading">
        <EmptyState title={copy.loading} headingLevel={2} isCompact />
      </SafeSection>
    )
  }
  const offline = error instanceof Error && "kind" in error && error.kind === "offline"
  if (error && rows.length === 0) {
    return (
      <SafeSection variant="transparent" padding={0} data-testid={offline ? "task-list-offline" : "task-list-error"}>
        <Banner
          status={offline ? "warning" : "error"}
          role={offline ? "status" : "alert"}
          title={offline ? copy.offline : copy.error}
          description={!offline ? error.message : undefined}
          endContent={onRetry ? <Button label={copy.retry} variant="ghost" size="sm" onClick={onRetry} /> : undefined}
        />
      </SafeSection>
    )
  }

  return (
    <SafeSection variant="transparent" padding={0} data-testid="task-list" data-density={density} role="region" aria-labelledby="task-list-heading" className="min-w-0">
      <SafeVStack className="min-w-0 gap-4">
        <SafeHStack as="header" justify="between" align="end" className="min-w-0 gap-4 flex-wrap">
          <SafeVStack className="min-w-0 gap-1">
            <Heading level={2} id="task-list-heading" className={showHeading ? undefined : "sr-only"}>{copy.title}</Heading>
            <Text type="supporting" hasTabularNumbers>{loading ? copy.refreshing : listRange(state.meta)}</Text>
          </SafeVStack>
          <SafeHStack align="end" className="gap-2 flex-wrap">
            {showToolbarSearch ? <TextInput label={copy.search} value={state.query.search} placeholder={copy.searchPlaceholder} onChange={(value) => updateQuery(state.query, onQueryChange, { search: value })} isLabelHidden htmlName="task-search" data-testid="list-search" /> : null}
            {onCreate ? <Button label={copy.createTask} variant="primary" size="sm" isDisabled={isMutationPending} onClick={(event) => onCreate(event.currentTarget)} data-testid="task-create" /> : null}
          </SafeHStack>
        </SafeHStack>

        <SafeSection variant="transparent" padding={0} className="min-w-0">
          <SafeHStack
            as="div"
            role="toolbar"
            aria-label={copy.filters}
            aria-orientation="horizontal"
            tabIndex={-1}
            id="task-list-controls"
            className="min-w-0 gap-2 flex-wrap"
            data-testid="task-attention-lens"
            data-count-scope="loaded-results"
          >
              <SafeHStack as="div" align="center" className="gap-1 flex-wrap" role="group" aria-label={copy.attentionLens}>
                <Text type="supporting">{copy.attentionLens}</Text>
                <Text type="supporting">{copy.attentionScope}</Text>
                {attentionLenses.map((lens) => (
                  <Button
                    key={lens}
                    label={copy.attentionLabels[lens]}
                    variant={selectedAttention === lens ? "primary" : "secondary"}
                    size="sm"
                    aria-pressed={selectedAttention === lens}
                    data-testid={`attention-lens-${lens}`}
                    endContent={<Text type="supporting" hasTabularNumbers data-testid={`attention-count-${lens}`}>{attentionCountsByStatus[lens]}</Text>}
                    onClick={() => onQueryChange(queryWithAttentionLens(state.query, lens))}
                  />
                ))}
                {selectedAttention !== null ? (
                  <SafeHStack as="div" align="center" className="gap-1" data-testid="attention-active-filter">
                    <Text type="supporting">{copy.activeAttention(copy.attentionLabels[selectedAttention])}</Text>
                    <Button label={copy.clearAttention} variant="ghost" size="sm" data-testid="attention-clear" onClick={() => updateQuery(state.query, onQueryChange, { status: [] })} />
                  </SafeHStack>
                ) : null}
              </SafeHStack>
              <Selector
                label={copy.status}
                options={[
                  { value: "all", label: copy.allStatuses },
                  ...statuses.map((status) => ({ value: status, label: `${copy.statusValues[status]}${status in attentionCountsByStatus ? ` · ${attentionCountsByStatus[status as AttentionLens]}` : ""}` })),
                ]}
                value={state.query.status[0] ?? "all"}
                onChange={(value) => updateQuery(state.query, onQueryChange, { status: value === "all" ? [] : [value as TaskListStatus] })}
                placeholder={copy.allStatuses}
                loadingText={copy.loading}
                size="sm"
                htmlName="task-status"
                data-testid="list-status-filter"
              />
              <MultiSelector label={copy.priority} options={priorities.map((priority) => ({ value: String(priority), label: `P${priority}` }))} value={state.query.priority.map(String)} onChange={(value) => updateQuery(state.query, onQueryChange, { priority: value.map(Number) })} placeholder={copy.allPriorities} loadingText={copy.loading} noOptionsText={copy.noMatches} triggerDisplay="labels" htmlName="task-priority" size="sm" />
              <MultiSelector label={copy.plan} options={plans.map((plan) => ({ value: plan, label: copy.planValues[plan] }))} value={[...state.query.plan]} onChange={(value) => updateQuery(state.query, onQueryChange, { plan: value as TaskListPlanFilter[] })} placeholder={copy.allPlans} loadingText={copy.loading} noOptionsText={copy.noMatches} triggerDisplay="labels" htmlName="task-plan" size="sm" />
              <Selector label={copy.sort} options={sorts.map((sort) => ({ value: sort, label: sort }))} value={state.query.sort} onChange={(value) => updateQuery(state.query, onQueryChange, { sort: value as TaskListSort })} placeholder={copy.sort} loadingText={copy.loading} size="sm" htmlName="task-sort" data-testid="list-sort" />
              <Selector label={copy.pageSize} options={[25, 50, 100, 200].map((limit) => ({ value: String(limit), label: String(limit) }))} value={String(state.query.limit)} onChange={(value) => updateQuery(state.query, onQueryChange, { limit: Number(value) })} placeholder={copy.pageSize} loadingText={copy.loading} size="sm" htmlName="task-limit" data-testid="list-limit" />
              <CheckboxInput label={copy.includeArchived} value={state.query.includeArchived} onChange={(value) => updateQuery(state.query, onQueryChange, { includeArchived: value })} htmlName="task-include-archived" size="sm" />
            <Button label={copy.reset} variant="ghost" size="sm" onClick={() => onQueryChange({ status: [], priority: [], plan: [], search: "", sort: "updated_at", page: 1, limit: 100, includeArchived: false })} />
          </SafeHStack>
        </SafeSection>

        {error ? (
          <Banner
            status={offline ? "warning" : "error"}
            role={offline ? "status" : "alert"}
            title={offline ? copy.offline : copy.error}
            description={!offline ? error.message : undefined}
            endContent={onRetry ? <Button label={copy.retry} variant="ghost" size="sm" onClick={onRetry} /> : undefined}
            data-testid={offline ? "task-list-offline" : "task-list-error"}
          />
        ) : null}

        {rows.length === 0 && !loading ? (
          <SafeSection variant="transparent" padding={0} data-testid="task-list-empty" data-empty-kind={emptyKind}>
            <EmptyState
              title={emptyKind === "filter" ? copy.noMatches : emptyKind === "page" ? copy.emptyPage : copy.emptyBoard}
              headingLevel={3}
              isCompact
              data-testid={`task-list-${emptyKind}-empty`}
            />
          </SafeSection>
        ) : null}

        {rows.length > 0 ? displayVariant === "grouped" ? (
          <SafeVStack className="gap-3" role="region" aria-label={copy.table} data-display-variant="list">
            {statuses.map((status) => {
              const group = rows.filter((task) => task.status === status)
              if (group.length === 0) return null
              return (
                <SafeSection key={status} variant="transparent" padding={0} dividers={["top", "bottom"]} role="group" aria-labelledby={`task-status-${status}`}>
                  <List
                    density={tableDensity(density)}
                    hasDividers
                    header={(
                      <SafeHStack as="div" justify="between" align="center" className="gap-2 p-2">
                        <Heading level={3} id={`task-status-${status}`}>{copy.statusValues[status]}</Heading>
                        <Text type="supporting" hasTabularNumbers>{group.length}</Text>
                      </SafeHStack>
                    )}
                  >
                    {group.map((task) => (
                      <ListItem
                        key={task.id}
                        data-testid="task-row"
                        data-task-id={task.id}
                        label={(
                          <SafeHStack as="div" align="center" className="gap-2 flex-wrap">
                            <Text type="code"><span translate="no">{task.ref}</span></Text>
                            <Button label={task.title} variant="ghost" size="sm" data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)}>
                              {task.title}
                            </Button>
                          </SafeHStack>
                        )}
                        endContent={(
                          <SafeHStack as="div" align="center" justify="end" className="gap-2 flex-wrap">
                            {visibleColumns?.priority !== false ? <Text type="supporting" hasTabularNumbers aria-label={`${copy.headers[3]} P${task.priority}`}>P{task.priority}</Text> : null}
                            {visibleColumns?.assignee !== false ? <Text type="supporting" aria-label={`${copy.headers[4]} ${task.assignee || "—"}`}>{task.assignee || "—"}</Text> : null}
                            {visibleColumns?.plan !== false ? <Text type="supporting" aria-label={`${copy.headers[5]} ${copy.planState[task.executionPlanState]}`}>{copy.planState[task.executionPlanState]}</Text> : null}
                            {visibleColumns?.steps !== false ? <Text type="supporting" hasTabularNumbers aria-label={`${copy.headers[6]} ${task.completedRequiredStepCount} / ${task.requiredStepCount}`}>{task.completedRequiredStepCount} / {task.requiredStepCount}{task.optionalStepCount ? ` + ${task.optionalStepCount}` : ""}</Text> : null}
                            {visibleColumns?.updated !== false ? <Text type="code" hasTabularNumbers aria-label={`${copy.headers[7]} ${task.updatedAt}`}>{task.updatedAt}</Text> : null}
                            {task.dependencyBlocked ? <Text type="supporting" color="secondary" aria-label={copy.statusValues.blocked}>{copy.blocked}</Text> : null}
                          </SafeHStack>
                        )}
                      />
                    ))}
                  </List>
                </SafeSection>
              )
            })}
          </SafeVStack>
        ) : (
          <Table<Record<string, unknown>>
            density={tableDensity(density)}
            dividers="rows"
            hasHover
            verticalAlign="top"
            textOverflow="truncate"
            aria-label={copy.table}
            data-display-variant="table"
            data-text-overflow="truncate"
            rowIndexStart={state.meta.offset + 1}
            rowCount={state.meta.total}
          >
            <caption className="sr-only">{copy.table}</caption>
            <TableHeader>
              <TableRow isHeaderRow>
                {[
                  ["ref", copy.headers[0]],
                  ["title", copy.headers[1]],
                  ["status", copy.headers[2]],
                  ["priority", copy.headers[3]],
                  ["assignee", copy.headers[4]],
                  ["plan", copy.headers[5]],
                  ["steps", copy.headers[6]],
                  ["updated", copy.headers[7]],
                ].filter(([id]) => id === "ref" || id === "title" || id === "status" || visibleColumns?.[id] !== false).map(([id, header]) => {
                  const column = id as TaskTableColumn
                  return <TableHeaderCell key={id} scope="col" data-column-key={id} className={tableColumnClasses[column]}>{header}</TableHeaderCell>
                })}
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((task, rowIndex) => {
                const cells: readonly [string, ReactNode][] = [
                  ["ref", <Text type="code" key="ref"><span translate="no">{task.ref}</span></Text>],
                  ["title", <Button label={task.title} variant="ghost" size="sm" className="min-w-0 max-w-full truncate" data-task-opener={taskOpenerKey(task.id)} onClick={() => onSelectTask(task.id)} key="title">{task.title}</Button>],
                  ["status", <SafeHStack as="div" align="center" className="gap-1 flex-wrap" key="status"><Badge variant={statusBadgeVariant(task.status)} label={copy.statusValues[task.status]} />{task.dependencyBlocked ? <Text type="supporting" color="secondary">{copy.blocked}</Text> : null}</SafeHStack>],
                  ["priority", <Text type="supporting" hasTabularNumbers key="priority">P{task.priority}</Text>],
                  ["assignee", <Text type="supporting" key="assignee">{task.assignee || "—"}</Text>],
                  ["plan", <Text type="supporting" key="plan">{copy.planState[task.executionPlanState]}</Text>],
                  ["steps", <Text type="supporting" hasTabularNumbers key="steps">{task.completedRequiredStepCount} / {task.requiredStepCount}{task.optionalStepCount ? ` + ${task.optionalStepCount}` : ""}</Text>],
                  ["updated", <Text type="code" hasTabularNumbers key="updated">{task.updatedAt}</Text>],
                ]
                return <TableRow key={task.id} data-testid="task-row" data-task-id={task.id} aria-rowindex={state.meta.offset + rowIndex + 1}>{cells.filter(([id]) => id === "ref" || id === "title" || id === "status" || visibleColumns?.[id] !== false).map(([id, cell]) => <TableCell key={id} className={tableColumnClasses[id as TaskTableColumn]}>{cell}</TableCell>)}</TableRow>
              })}
            </TableBody>
          </Table>
        ) : null}

        <SafeHStack as="footer" justify="center" align="center" className="gap-3 flex-wrap" aria-label={copy.table}>
          <Button label={copy.previous} variant="secondary" size="sm" isDisabled={!canPrevious} onClick={() => updateQuery(state.query, onQueryChange, { page: currentPage - 1 })} />
          <Text type="supporting" hasTabularNumbers>{copy.page} {currentPage}{copy.pageSuffix} / {totalPages}</Text>
          <Button label={copy.next} variant="secondary" size="sm" isDisabled={!canNext} onClick={() => updateQuery(state.query, onQueryChange, { page: currentPage + 1 })} />
        </SafeHStack>
      </SafeVStack>
    </SafeSection>
  )
}
