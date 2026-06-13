import {
  type ColumnDef,
  type RowSelectionState,
  type VisibilityState,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table"
import { CalendarClock, ChevronsLeft, ChevronsRight, Eye, MoreHorizontal, Rows3 } from "lucide-react"
import { useMemo, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
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
  filterListTasks,
  listColumnLabels,
  selectedRowCount,
  type ListColumnId,
  type PriorityFilter,
} from "./table-state"

export function ListView({
  tasks,
  selectedId,
  page,
  hasNextPage,
  hasPreviousPage,
  canGoLastPage,
  rowsPerPage,
  tasksRefreshing,
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
  tasksRefreshing: boolean
  onSelectTask: (taskId: string) => void
  onFirstPage: () => void
  onPreviousPage: () => void
  onNextPage: () => void
  onLastPage: () => void
  onRowsPerPageChange: (value: number) => void
}) {
  const [tableStatus, setTableStatus] = useState<TaskStatus | "all">("all")
  const [priorityFilter, setPriorityFilter] = useState<PriorityFilter>("all")
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({})
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>(defaultListColumnVisibility)

  const filteredTasks = useMemo(
    () => filterListTasks(tasks, tableStatus, priorityFilter),
    [priorityFilter, tableStatus, tasks],
  )

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
        header: "Ref",
        cell: ({ row }) => (
          <div>
            <div className="font-medium text-neutral-800">{row.original.ref || `#${row.original.seq}`}</div>
            <div className="text-xs text-neutral-500">{shortId(row.original.id)}</div>
          </div>
        ),
      },
      {
        accessorKey: "title",
        header: "Title",
        cell: ({ row }) => (
          <div className="min-w-0">
            <button
              className="block max-w-full truncate text-left font-medium text-neutral-950 underline-offset-2 hover:underline focus:outline-none focus:ring-2 focus:ring-neutral-400"
              onClick={() => onSelectTask(row.original.id)}
            >
              {row.original.title}
            </button>
            {row.original.status_reason ? (
              <div className="truncate text-xs text-neutral-500">{row.original.status_reason}</div>
            ) : null}
          </div>
        ),
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => <Badge variant={badgeVariant(row.original.status)}>{row.original.status}</Badge>,
      },
      {
        accessorKey: "priority",
        header: "Priority",
        cell: ({ row }) => (
          <Badge variant="secondary" className={priorityBadgeClass(row.original.priority)}>
            {priorityLabel(row.original.priority)}
          </Badge>
        ),
      },
      {
        accessorKey: "assignee",
        header: "Assignee",
        cell: ({ row }) => (
          <span className="truncate text-neutral-600">{row.original.assignee ?? row.original.claim_owner ?? "-"}</span>
        ),
      },
      {
        id: "schedule",
        header: "Scheduled / due",
        cell: ({ row }) => (
          <div className="space-y-0.5 text-xs text-neutral-600">
            <DateLine label="s" value={row.original.scheduled_at} />
            <DateLine label="d" value={row.original.due_at} />
          </div>
        ),
      },
      {
        id: "updated",
        accessorKey: "updated_at",
        header: "Updated",
        cell: ({ row }) => (
          <span className="whitespace-nowrap text-xs text-neutral-500">{formatRelativeTime(row.original.updated_at)}</span>
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
    [onSelectTask],
  )

  const table = useReactTable({
    data: filteredTasks,
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

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-white">
      <div className="flex flex-wrap items-center gap-2 border-b border-neutral-200 px-4 py-3">
        <label className="flex items-center gap-2 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs text-neutral-600">
          Status
          <select
            className="bg-transparent text-neutral-950 outline-none"
            value={tableStatus}
            onChange={(event) => setTableStatus(event.target.value as TaskStatus | "all")}
          >
            <option value="all">all page rows</option>
            {filterStatuses.map((status) => (
              <option key={status} value={status}>
                {status}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-2 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs text-neutral-600">
          Priority
          <select
            className="bg-transparent text-neutral-950 outline-none"
            value={priorityFilter}
            onChange={(event) => {
              const value = event.target.value
              setPriorityFilter(value === "all" ? "all" : (Number(value) as PriorityFilter))
            }}
          >
            <option value="all">all</option>
            {priorityLevels.map((priority) => (
              <option key={priority} value={priority}>
                {priorityLabel(priority)}
              </option>
            ))}
          </select>
        </label>
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
        <div className="text-xs text-neutral-500">
          {selectedCount} selected · {filteredTasks.length} current-page rows
        </div>
        <div className="ml-auto flex items-center gap-2 text-xs text-neutral-500">
          <span>{pageRangeLabel(page, tasks.length)}</span>
          {tasksRefreshing ? <span>refreshing</span> : null}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full min-w-[980px] border-separate border-spacing-0 text-sm">
          <thead className="sticky top-0 z-10 bg-neutral-50 text-xs font-medium uppercase tracking-normal text-neutral-500">
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <th key={header.id} className="border-b border-neutral-200 px-3 py-2 text-left font-medium">
                    {header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.length ? (
              table.getRowModel().rows.map((row) => (
                <tr
                  key={row.id}
                  className={cn(
                    "border-b border-neutral-100 hover:bg-neutral-50",
                    selectedId === row.original.id && "bg-neutral-100",
                  )}
                >
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id} className="max-w-[320px] border-b border-neutral-100 px-3 py-2 align-middle">
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
              ))
            ) : (
              <tr>
                <td className="px-4 py-10 text-center text-sm text-neutral-500" colSpan={table.getAllLeafColumns().length}>
                  No tasks match the current page filters.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex h-10 items-center gap-2 border-t border-neutral-200 bg-white px-4 text-xs text-neutral-500">
        <label className="flex items-center gap-2">
          Rows
          <select
            className="rounded-md border border-neutral-200 bg-white px-2 py-1 text-neutral-950 outline-none"
            value={rowsPerPage}
            onChange={(event) => onRowsPerPageChange(Number(event.target.value))}
          >
            {[25, 50, 100, 200].map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </label>
        <div className="ml-auto flex items-center gap-1">
          <Button variant="ghost" size="icon" disabled={!hasPreviousPage || tasksRefreshing} onClick={onFirstPage}>
            <ChevronsLeft className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="sm" disabled={!hasPreviousPage || tasksRefreshing} onClick={onPreviousPage}>
            Previous
          </Button>
          <Button variant="ghost" size="sm" disabled={!hasNextPage || tasksRefreshing} onClick={onNextPage}>
            Next
          </Button>
          <Button variant="ghost" size="icon" disabled={!canGoLastPage || tasksRefreshing} onClick={onLastPage}>
            <ChevronsRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}

function DateLine({ label, value }: { label: string; value: number | null }) {
  return (
    <div className="flex items-center gap-1">
      <CalendarClock className="h-3 w-3 text-neutral-400" />
      <span className="font-medium text-neutral-400">{label}</span>
      <span>{value ? formatRelativeTime(value) : "-"}</span>
    </div>
  )
}

function badgeVariant(status: Task["status"]) {
  if (status === "ready" || status === "done") return "ready"
  if (status === "running") return "running"
  if (status === "blocked") return "blocked"
  if (status === "review") return "review"
  return "secondary"
}
