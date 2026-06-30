import {
  type ColumnDef,
  type RowSelectionState,
  type VisibilityState,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table"
import { ArrowDown, ArrowUp, CalendarClock, ChevronsLeft, ChevronsRight, Eye, MoreHorizontal, Rows3, X } from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { PageToolbar, PriorityBadge, TaskIdentityLine, TaskStatusBadge } from "@/components/ui/composites"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Label } from "@/components/ui/label"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { Pagination, PaginationContent, PaginationItem } from "@/components/ui/pagination"
import { Progress } from "@/components/ui/progress"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip"
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { filterStatuses } from "@/features/board/board-config"
import type { PageMeta, Task, TaskStatus } from "@/lib/api"
import { pageRangeLabel } from "@/lib/pagination"
import { priorityLabel, priorityLevels } from "@/lib/priority"
import { cn, formatRelativeTime } from "@/lib/utils"
import { useI18n } from "@/i18n"

import {
  defaultListColumnVisibility,
  hasActiveListFilters,
  listColumnLabels,
  selectedRowCount,
  sortForColumn,
  stepProgressForTask,
  togglePlanFilter,
  togglePriorityFilter,
  type ListColumnId,
  type TaskPlanFilter,
  type ListSortDirection,
  type ListSortState,
} from "./table-state"

const rowsPerPageOptions: MenuSelectOption<string>[] = [25, 50, 100, 200].map((value) => ({
  value: String(value),
  label: String(value),
}))

export function ListView({
  tasks,
  selectedId,
  page,
  hasNextPage,
  hasPreviousPage,
  canGoLastPage,
  rowsPerPage,
  statusFilter,
  priorityFilters,
  planFilters,
  listSort,
  tasksRefreshing,
  onStatusFilterChange,
  onPriorityFiltersChange,
  onPlanFiltersChange,
  onListSortChange,
  onResetListFilters,
  onSelectTask,
  onFirstPage,
  onPreviousPage,
  onNextPage,
  onLastPage,
  onRowsPerPageChange,
}: {
  tasks: Task[]
  selectedId: string | null
  page: PageMeta
  hasNextPage: boolean
  hasPreviousPage: boolean
  canGoLastPage: boolean
  rowsPerPage: number
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  planFilters: TaskPlanFilter[]
  listSort: ListSortState
  tasksRefreshing: boolean
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onPriorityFiltersChange: (value: number[]) => void
  onPlanFiltersChange: (value: TaskPlanFilter[]) => void
  onListSortChange: (value: ListSortState) => void
  onResetListFilters: () => void
  onSelectTask: (taskId: string) => void
  onFirstPage: () => void
  onPreviousPage: () => void
  onNextPage: () => void
  onLastPage: () => void
  onRowsPerPageChange: (value: number) => void
}) {
  const { t } = useI18n()
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({})
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>(defaultListColumnVisibility)
  const selectTaskRef = useRef(onSelectTask)
  selectTaskRef.current = onSelectTask
  const statusFilterOptions = useMemo<MenuSelectOption<TaskStatus | "all">[]>(
    () => [
      { value: "all", label: t("all active") },
      ...filterStatuses.map((status) => ({ value: status, label: t(status) })),
    ],
    [t],
  )
  const planFilterOptions = useMemo<{ value: TaskPlanFilter; label: string }[]>(
    () => [
      { value: "plan_needed", label: t("Plan needed") },
      { value: "has_steps", label: t("Has steps") },
      { value: "incomplete_required_steps", label: t("Incomplete required") },
    ],
    [t],
  )

  useEffect(() => {
    const taskIds = new Set(tasks.map((task) => task.id))
    setRowSelection((current) => {
      let changed = false
      const next: RowSelectionState = {}
      for (const [taskId, selected] of Object.entries(current)) {
        if (!taskIds.has(taskId)) {
          changed = true
          continue
        }
        next[taskId] = selected
      }
      return changed ? next : current
    })
  }, [tasks])

  const columns = useMemo<ColumnDef<Task>[]>(
    () => [
      {
        id: "select",
        header: ({ table }) => (
          <Checkbox
            aria-label={t("Select all visible rows")}
            checked={table.getIsAllPageRowsSelected() || (table.getIsSomePageRowsSelected() && "indeterminate")}
            onCheckedChange={(value) => table.toggleAllPageRowsSelected(Boolean(value))}
          />
        ),
        cell: ({ row }) => (
          <Checkbox
            aria-label={t("Select {ref}", { ref: row.original.ref })}
            checked={row.getIsSelected()}
            onCheckedChange={(value) => row.toggleSelected(Boolean(value))}
            onClick={(event) => event.stopPropagation()}
          />
        ),
        enableHiding: false,
      },
      {
        accessorKey: "ref",
        header: ({ column }) => (
          <SortableHeader columnId="ref" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => (
          <TaskIdentityLine
            id={row.original.id}
            ref={row.original.ref}
            seq={row.original.seq}
            className="[&>div:first-child]:text-foreground"
          />
        ),
      },
      {
        accessorKey: "title",
        header: ({ column }) => (
          <SortableHeader columnId="title" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => (
          <div className="min-w-0">
            <Button
              type="button"
              variant="ghost"
              className="h-auto max-w-full justify-start truncate px-0 py-0 text-left font-medium text-foreground underline-offset-2 hover:bg-transparent hover:underline"
              onClick={() => selectTaskRef.current(row.original.id)}
            >
              {row.original.title}
            </Button>
            {row.original.status_reason ? (
              <div className="truncate text-xs text-muted-foreground">{row.original.status_reason}</div>
            ) : null}
            {row.original.labels.length ? (
              <div className="mt-1 flex max-w-full flex-wrap gap-1">
                {row.original.labels.map((label) => (
                  <Badge key={label.id} variant="secondary" className="max-w-32 truncate px-1.5 py-0 text-[11px] leading-5">
                    {label.name}
                  </Badge>
                ))}
              </div>
            ) : null}
          </div>
        ),
      },
      {
        accessorKey: "status",
        header: ({ column }) => (
          <SortableHeader columnId="status" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => <TaskStatusBadge status={row.original.status} />,
      },
      {
        accessorKey: "priority",
        header: ({ column }) => (
          <SortableHeader columnId="priority" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => <PriorityBadge priority={row.original.priority} />,
      },
      {
        accessorKey: "assignee",
        header: ({ column }) => (
          <SortableHeader columnId="assignee" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => (
          <span className="truncate text-muted-foreground">{row.original.assignee ?? row.original.claim_owner ?? "-"}</span>
        ),
      },
      {
        id: "execution_plan",
        header: ({ column }) => <StaticHeader columnId="execution_plan" onHide={() => column.toggleVisibility(false)} />,
        cell: ({ row }) => <ExecutionPlanBadge task={row.original} />,
      },
      {
        id: "step_progress",
        header: ({ column }) => <StaticHeader columnId="step_progress" onHide={() => column.toggleVisibility(false)} />,
        cell: ({ row }) => <StepProgressCell task={row.original} />,
      },
      {
        id: "dependency_blocked",
        header: ({ column }) => <StaticHeader columnId="dependency_blocked" onHide={() => column.toggleVisibility(false)} />,
        cell: ({ row }) => <DependencyBlockedBadge task={row.original} />,
      },
      {
        id: "schedule",
        header: ({ column }) => (
          <SortableHeader columnId="schedule" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => (
          <div className="space-y-0.5 text-xs text-muted-foreground">
            <DateLine label="s" value={row.original.scheduled_at} />
            <DateLine label="d" value={row.original.due_at} />
          </div>
        ),
      },
      {
        id: "updated",
        accessorKey: "updated_at",
        header: ({ column }) => (
          <SortableHeader columnId="updated" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => (
          <span className="whitespace-nowrap text-xs text-muted-foreground">{formatRelativeTime(row.original.updated_at)}</span>
        ),
      },
      {
        id: "actions",
        enableHiding: false,
        header: "",
        cell: ({ row }) => (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                aria-label={t("Actions for {ref}", { ref: row.original.ref })}
                onClick={(event) => event.stopPropagation()}
              >
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onSelect={() => selectTaskRef.current(row.original.id)}>
                <Eye className="h-4 w-4" />
                {t("Open detail")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ),
      },
    ],
    [listSort, onListSortChange, t],
  )

  const table = useReactTable({
    data: tasks,
    columns,
    getRowId: (row) => row.id,
    getCoreRowModel: getCoreRowModel(),
    onRowSelectionChange: setRowSelection,
    onColumnVisibilityChange: setColumnVisibility,
    state: {
      rowSelection,
      columnVisibility,
    },
  })

  const selectedCount = selectedRowCount(rowSelection)
  const activeFilters = hasActiveListFilters("", statusFilter, priorityFilters, planFilters)

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-card">
      <PageToolbar>
        <div className="flex items-center gap-2">
          <Label className="text-xs text-muted-foreground">{t("Status")}</Label>
          <MenuSelect
            ariaLabel={t("List status filter")}
            value={statusFilter}
            options={statusFilterOptions}
            onValueChange={onStatusFilterChange}
            triggerClassName="min-w-28"
          />
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm">
              {t("Priority")}
              {priorityFilters.length ? ` (${priorityFilters.length})` : ""}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {priorityLevels.map((priority) => (
              <DropdownMenuCheckboxItem
                key={priority}
                checked={priorityFilters.includes(priority)}
                onCheckedChange={() => onPriorityFiltersChange(togglePriorityFilter(priorityFilters, priority))}
              >
                {priorityLabel(priority)}
              </DropdownMenuCheckboxItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm">
              {t("Plan")}
              {planFilters.length ? ` (${planFilters.length})` : ""}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {planFilterOptions.map((option) => (
              <DropdownMenuCheckboxItem
                key={option.value}
                checked={planFilters.includes(option.value)}
                onCheckedChange={() => onPlanFiltersChange(togglePlanFilter(planFilters, option.value))}
              >
                {option.label}
              </DropdownMenuCheckboxItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <Button variant="ghost" size="sm" disabled={!activeFilters} onClick={onResetListFilters}>
          <X className="h-3.5 w-3.5" />
          {t("Reset")}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm">
              <Rows3 className="h-3.5 w-3.5" />
              {t("View")}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {table
              .getAllColumns()
              .filter((column) => column.getCanHide())
              .map((column) => (
                <DropdownMenuCheckboxItem
                  key={column.id}
                  checked={column.getIsVisible()}
                  onCheckedChange={(value) => column.toggleVisibility(Boolean(value))}
                >
                  {t(listColumnLabels[column.id as ListColumnId] ?? column.id)}
                </DropdownMenuCheckboxItem>
              ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => setColumnVisibility(defaultListColumnVisibility)}>
              {t("Reset columns")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <div className="text-xs text-muted-foreground">
          {t("{count} selected", { count: selectedCount })} · {t("{count} rows", { count: tasks.length })}
        </div>
        <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          <span>{pageRangeLabel(page, tasks.length)}</span>
          {tasksRefreshing ? <span>{t("refreshing")}</span> : null}
        </div>
      </PageToolbar>

      <ScrollArea className="min-w-0 flex-1">
        <Table className="min-w-[980px] border-separate border-spacing-0">
          <TableHeader className="sticky top-0 z-10 bg-muted">
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id} className="hover:bg-transparent">
                {headerGroup.headers.map((header) => (
                  <TableHead key={header.id} className="border-b border-border">
                    {header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows.length ? (
              table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  className={cn(
                    selectedId === row.original.id && "bg-muted",
                  )}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id} className="max-w-[320px] border-b border-border">
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell className="px-4 py-10" colSpan={table.getAllLeafColumns().length}>
                  <Empty className="p-0">
                    <EmptyDescription>{t("No tasks match the current filters.")}</EmptyDescription>
                  </Empty>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </ScrollArea>

      <div className="flex h-10 items-center gap-2 border-t border-border bg-card px-4 text-xs text-muted-foreground">
        <Label className="flex items-center gap-2" id="rows-per-page-label">
          {t("Rows")}
        </Label>
        <MenuSelect
          ariaLabel={t("Rows per page")}
          value={String(rowsPerPage)}
          options={rowsPerPageOptions}
          onValueChange={(value) => onRowsPerPageChange(Number(value))}
          triggerClassName="h-7 min-w-20"
        />
        <Pagination className="ml-auto">
          <PaginationContent>
            <PaginationItem>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={t("First page")}
                    disabled={!hasPreviousPage || tasksRefreshing}
                    onClick={onFirstPage}
                  >
                    <ChevronsLeft className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("First page")}</TooltipContent>
              </Tooltip>
            </PaginationItem>
            <PaginationItem>
              <Button variant="ghost" size="sm" disabled={!hasPreviousPage || tasksRefreshing} onClick={onPreviousPage}>
                {t("Previous")}
              </Button>
            </PaginationItem>
            <PaginationItem>
              <Button variant="ghost" size="sm" disabled={!hasNextPage || tasksRefreshing} onClick={onNextPage}>
                {t("Next")}
              </Button>
            </PaginationItem>
            <PaginationItem>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={t("Last page")}
                    disabled={!canGoLastPage || tasksRefreshing}
                    onClick={onLastPage}
                  >
                    <ChevronsRight className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{t("Last page")}</TooltipContent>
              </Tooltip>
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </div>
    </div>
  )
}

function ExecutionPlanBadge({ task }: { task: Task }) {
  const { t } = useI18n()
  if (task.execution_plan_state === "planned") {
    return <Badge variant="secondary">{t("steps {completed}/{total}", { completed: task.completed_required_step_count, total: task.required_step_count })}</Badge>
  }
  if (task.execution_plan_state === "not_required") return <Badge variant="secondary">{t("not required")}</Badge>
  return <Badge variant="blocked">{t("plan needed")}</Badge>
}

function StepProgressCell({ task }: { task: Task }) {
  const { t } = useI18n()
  const progress = stepProgressForTask(task)
  if (!progress) return <span className="text-xs text-muted-foreground">-</span>

  const label = t("{completed}/{total} required steps", { completed: progress.completed, total: progress.total })

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          aria-label={t("Required step progress: {label}", { label })}
          className="inline-flex w-32 items-center gap-2 rounded-sm text-xs text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          tabIndex={0}
        >
          <Progress
            aria-label={t("Required step progress: {label}", { label })}
            aria-valuemax={100}
            aria-valuemin={0}
            aria-valuenow={progress.percent}
            className="h-2 w-20 shrink-0 bg-muted"
            role="progressbar"
            value={progress.percent}
          />
          <span className="font-medium text-foreground">{progress.completed}/{progress.total}</span>
        </span>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

function DependencyBlockedBadge({ task }: { task: Task }) {
  const { t } = useI18n()
  if (!task.dependency_blocked) return <span className="text-xs text-muted-foreground">-</span>
  return <Badge variant="blocked">{t("blocked by {count}", { count: task.unfinished_parent_count })}</Badge>
}

function DateLine({ label, value }: { label: string; value: number | null }) {
  return (
    <div className="flex items-center gap-1">
      <CalendarClock className="h-3 w-3 text-muted-foreground" />
      <span className="font-medium text-muted-foreground">{label}</span>
      <span>{value ? formatRelativeTime(value) : "-"}</span>
    </div>
  )
}

function StaticHeader({ columnId, onHide }: { columnId: ListColumnId; onHide: () => void }) {
  const { t } = useI18n()
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="h-7 px-1.5 text-xs uppercase">
          {t(listColumnLabels[columnId])}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuItem onSelect={onHide}>{t("Hide")}</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function SortableHeader({
  columnId,
  listSort,
  onListSortChange,
  onHide,
}: {
  columnId: ListColumnId
  listSort: ListSortState
  onListSortChange: (value: ListSortState) => void
  onHide: () => void
}) {
  const { t } = useI18n()
  const field = sortForColumn(columnId)
  const active = field ? listSort.field === field : false
  const label = t(listColumnLabels[columnId])

  function setDirection(direction: ListSortDirection) {
    if (!field) return
    onListSortChange({ field, direction })
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="h-7 px-1.5 text-xs uppercase">
          {label}
          {active && listSort.direction === "asc" ? <ArrowUp className="h-3.5 w-3.5" /> : null}
          {active && listSort.direction === "desc" ? <ArrowDown className="h-3.5 w-3.5" /> : null}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuItem disabled={!field} onSelect={() => setDirection("asc")}>
          <ArrowUp className="h-4 w-4" />
          {t("Asc")}
        </DropdownMenuItem>
        <DropdownMenuItem disabled={!field} onSelect={() => setDirection("desc")}>
          <ArrowDown className="h-4 w-4" />
          {t("Desc")}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onHide}>{t("Hide")}</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
