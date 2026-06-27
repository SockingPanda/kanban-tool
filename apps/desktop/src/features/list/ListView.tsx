import {
  type ColumnDef,
  type RowSelectionState,
  type VisibilityState,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table"
import { ArrowDown, ArrowUp, CalendarClock, ChevronsLeft, ChevronsRight, Eye, MoreHorizontal, Rows3, X } from "lucide-react"
import { useMemo, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Empty, EmptyDescription } from "@/components/ui/empty"
import { Label } from "@/components/ui/label"
import { MenuSelect, type MenuSelectOption } from "@/components/ui/menu-select"
import { Pagination, PaginationContent, PaginationItem } from "@/components/ui/pagination"
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
import { priorityBadgeClass, priorityLabel, priorityLevels } from "@/lib/priority"
import { cn, formatRelativeTime, shortId } from "@/lib/utils"

import {
  defaultListColumnVisibility,
  hasActiveListFilters,
  listColumnLabels,
  selectedRowCount,
  sortForColumn,
  togglePlanFilter,
  togglePriorityFilter,
  type ListColumnId,
  type TaskPlanFilter,
  type ListSortDirection,
  type ListSortState,
} from "./table-state"

const statusFilterOptions: MenuSelectOption<TaskStatus | "all">[] = [
  { value: "all", label: "all active" },
  ...filterStatuses.map((status) => ({ value: status, label: status })),
]

const rowsPerPageOptions: MenuSelectOption<string>[] = [25, 50, 100, 200].map((value) => ({
  value: String(value),
  label: String(value),
}))

const planFilterOptions: { value: TaskPlanFilter; label: string }[] = [
  { value: "plan_needed", label: "Plan needed" },
  { value: "has_steps", label: "Has steps" },
  { value: "incomplete_required_steps", label: "Incomplete required" },
]

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
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({})
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>(defaultListColumnVisibility)

  const columns = useMemo<ColumnDef<Task>[]>(
    () => [
      {
        id: "select",
        header: ({ table }) => (
          <Checkbox
            aria-label="Select all visible rows"
            checked={table.getIsAllPageRowsSelected() || (table.getIsSomePageRowsSelected() && "indeterminate")}
            onCheckedChange={(value) => table.toggleAllPageRowsSelected(Boolean(value))}
          />
        ),
        cell: ({ row }) => (
          <Checkbox
            aria-label={`Select ${row.original.ref}`}
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
          <div>
            <div className="font-medium text-foreground">{row.original.ref || `#${row.original.seq}`}</div>
            <div className="text-xs text-muted-foreground">{shortId(row.original.id)}</div>
          </div>
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
              onClick={() => onSelectTask(row.original.id)}
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
        cell: ({ row }) => <Badge variant={badgeVariant(row.original.status)}>{row.original.status}</Badge>,
      },
      {
        accessorKey: "priority",
        header: ({ column }) => (
          <SortableHeader columnId="priority" listSort={listSort} onListSortChange={onListSortChange} onHide={() => column.toggleVisibility(false)} />
        ),
        cell: ({ row }) => (
          <Badge variant="secondary" className={priorityBadgeClass(row.original.priority)}>
            {priorityLabel(row.original.priority)}
          </Badge>
        ),
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
        id: "required_steps",
        header: ({ column }) => <StaticHeader columnId="required_steps" onHide={() => column.toggleVisibility(false)} />,
        cell: ({ row }) => <span className="text-xs text-muted-foreground">{row.original.required_step_count}</span>,
      },
      {
        id: "done_required_steps",
        header: ({ column }) => <StaticHeader columnId="done_required_steps" onHide={() => column.toggleVisibility(false)} />,
        cell: ({ row }) => <span className="text-xs text-muted-foreground">{row.original.completed_required_step_count}</span>,
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
                aria-label={`Actions for ${row.original.ref}`}
                onClick={(event) => event.stopPropagation()}
              >
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onSelect={() => onSelectTask(row.original.id)}>
                <Eye className="h-4 w-4" />
                Open detail
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ),
      },
    ],
    [listSort, onListSortChange, onSelectTask],
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
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3">
        <div className="flex items-center gap-2">
          <Label className="text-xs text-muted-foreground">Status</Label>
          <MenuSelect
            ariaLabel="List status filter"
            value={statusFilter}
            options={statusFilterOptions}
            onValueChange={onStatusFilterChange}
            triggerClassName="min-w-28"
          />
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm">
              Priority
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
              Plan
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
          Reset
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm">
              <Rows3 className="h-3.5 w-3.5" />
              View
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
                  {listColumnLabels[column.id as ListColumnId] ?? column.id}
                </DropdownMenuCheckboxItem>
              ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => setColumnVisibility(defaultListColumnVisibility)}>
              Reset columns
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <div className="text-xs text-muted-foreground">
          {selectedCount} selected · {tasks.length} rows
        </div>
        <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          <span>{pageRangeLabel(page, tasks.length)}</span>
          {tasksRefreshing ? <span>refreshing</span> : null}
        </div>
      </div>

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
                    <EmptyDescription>No tasks match the current filters.</EmptyDescription>
                  </Empty>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </ScrollArea>

      <div className="flex h-10 items-center gap-2 border-t border-border bg-card px-4 text-xs text-muted-foreground">
        <Label className="flex items-center gap-2" id="rows-per-page-label">
          Rows
        </Label>
        <MenuSelect
          ariaLabel="Rows per page"
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
                    aria-label="First page"
                    disabled={!hasPreviousPage || tasksRefreshing}
                    onClick={onFirstPage}
                  >
                    <ChevronsLeft className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>First page</TooltipContent>
              </Tooltip>
            </PaginationItem>
            <PaginationItem>
              <Button variant="ghost" size="sm" disabled={!hasPreviousPage || tasksRefreshing} onClick={onPreviousPage}>
                Previous
              </Button>
            </PaginationItem>
            <PaginationItem>
              <Button variant="ghost" size="sm" disabled={!hasNextPage || tasksRefreshing} onClick={onNextPage}>
                Next
              </Button>
            </PaginationItem>
            <PaginationItem>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="Last page"
                    disabled={!canGoLastPage || tasksRefreshing}
                    onClick={onLastPage}
                  >
                    <ChevronsRight className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Last page</TooltipContent>
              </Tooltip>
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </div>
    </div>
  )
}

function ExecutionPlanBadge({ task }: { task: Task }) {
  if (task.execution_plan_state === "planned") {
    return <Badge variant="secondary">steps {task.completed_required_step_count}/{task.required_step_count}</Badge>
  }
  if (task.execution_plan_state === "not_required") return <Badge variant="secondary">not required</Badge>
  return <Badge variant="blocked">plan needed</Badge>
}

function DependencyBlockedBadge({ task }: { task: Task }) {
  if (!task.dependency_blocked) return <span className="text-xs text-muted-foreground">-</span>
  return <Badge variant="blocked">blocked by {task.unfinished_parent_count}</Badge>
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
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="h-7 px-1.5 text-xs uppercase">
          {listColumnLabels[columnId]}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuItem onSelect={onHide}>Hide</DropdownMenuItem>
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
  const field = sortForColumn(columnId)
  const active = field ? listSort.field === field : false
  const label = listColumnLabels[columnId]

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
          Asc
        </DropdownMenuItem>
        <DropdownMenuItem disabled={!field} onSelect={() => setDirection("desc")}>
          <ArrowDown className="h-4 w-4" />
          Desc
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onHide}>Hide</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function badgeVariant(status: Task["status"]) {
  if (status === "ready" || status === "done") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  return "secondary"
}
