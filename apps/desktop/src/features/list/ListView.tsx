import {
  type ColumnDef,
  type RowSelectionState,
  type VisibilityState,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table"
import { ArrowDown, ArrowUp, CalendarClock, ChevronsLeft, ChevronsRight, Eye, MoreHorizontal, Rows3, Search, X } from "lucide-react"
import { useMemo, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { NativeSelect } from "@/components/ui/native-select"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Tooltip } from "@/components/ui/tooltip"
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
  togglePriorityFilter,
  type ListColumnId,
  type ListSortDirection,
  type ListSortState,
} from "./table-state"

export function ListView({
  tasks,
  selectedId,
  page,
  hasNextPage,
  hasPreviousPage,
  canGoLastPage,
  rowsPerPage,
  search,
  statusFilter,
  priorityFilters,
  listSort,
  tasksRefreshing,
  onSearchChange,
  onStatusFilterChange,
  onPriorityFiltersChange,
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
  search: string
  statusFilter: TaskStatus | "all"
  priorityFilters: number[]
  listSort: ListSortState
  tasksRefreshing: boolean
  onSearchChange: (value: string) => void
  onStatusFilterChange: (value: TaskStatus | "all") => void
  onPriorityFiltersChange: (value: number[]) => void
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
            <button
              className="block max-w-full truncate text-left font-medium text-foreground underline-offset-2 hover:underline focus:outline-none focus:ring-2 focus:ring-ring"
              onClick={() => onSelectTask(row.original.id)}
            >
              {row.original.title}
            </button>
            {row.original.status_reason ? (
              <div className="truncate text-xs text-muted-foreground">{row.original.status_reason}</div>
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
  const activeFilters = hasActiveListFilters(search, statusFilter, priorityFilters)

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-card">
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3">
        <div className="relative min-w-0 flex-1 basis-64 sm:w-72 sm:flex-none">
          <Search className="pointer-events-none absolute left-2.5 top-2 h-4 w-4 text-muted-foreground" />
          <Input
            className="pl-8"
            placeholder="Filter tasks"
            value={search}
            onChange={(event) => onSearchChange(event.target.value)}
          />
        </div>
        <Label className="flex items-center gap-2 rounded-md border border-border bg-background px-2 py-1">
          Status
          <NativeSelect
            className="h-6 border-0 bg-transparent px-0"
            value={statusFilter}
            onChange={(event) => onStatusFilterChange(event.target.value as TaskStatus | "all")}
          >
            <option value="all">all active</option>
            {filterStatuses.map((status) => (
              <option key={status} value={status}>
                {status}
              </option>
            ))}
          </NativeSelect>
        </Label>
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
                <TableCell className="px-4 py-10 text-center text-sm text-muted-foreground" colSpan={table.getAllLeafColumns().length}>
                  No tasks match the current filters.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </ScrollArea>

      <div className="flex h-10 items-center gap-2 border-t border-border bg-card px-4 text-xs text-muted-foreground">
        <Label className="flex items-center gap-2">
          Rows
          <NativeSelect
            className="h-7"
            value={rowsPerPage}
            onChange={(event) => onRowsPerPageChange(Number(event.target.value))}
          >
            {[25, 50, 100, 200].map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </NativeSelect>
        </Label>
        <div className="ml-auto flex items-center gap-1">
          <Tooltip content="First page">
            <Button variant="ghost" size="icon" disabled={!hasPreviousPage || tasksRefreshing} onClick={onFirstPage}>
              <ChevronsLeft className="h-4 w-4" />
            </Button>
          </Tooltip>
          <Button variant="ghost" size="sm" disabled={!hasPreviousPage || tasksRefreshing} onClick={onPreviousPage}>
            Previous
          </Button>
          <Button variant="ghost" size="sm" disabled={!hasNextPage || tasksRefreshing} onClick={onNextPage}>
            Next
          </Button>
          <Tooltip content="Last page">
            <Button variant="ghost" size="icon" disabled={!canGoLastPage || tasksRefreshing} onClick={onLastPage}>
              <ChevronsRight className="h-4 w-4" />
            </Button>
          </Tooltip>
        </div>
      </div>
    </div>
  )
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
