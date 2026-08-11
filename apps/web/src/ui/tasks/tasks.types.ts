import type { ReactNode } from "react"

import type { BoardTaskStatus, BoardTaskViewModel } from "../../features/board/types"
import type { TaskInspectorViewModel } from "../../features/explorer/TaskInspector"

/** Product-level task routes. Table is a List display variant, not a route. */
export type TasksView = "board" | "list" | "map"

/** Storybook-only unsupported projection; never use as a product route. */
export type UnsupportedTasksView = "timeline"

/** List's display projection; `table` never becomes a URL route. */
export type TasksListDisplay = "grouped" | "table"

export type TasksDensity = "dense" | "comfortable"

export type TasksLocale = "zh" | "en"

export interface TaskFilterValue {
  readonly id: string
  readonly label: string
  readonly removable?: boolean
}

interface TaskFilterBarCommonProps {
  readonly placeholder?: string
  /** Disable the search control when the active projection has no query-backed handler. */
  readonly disabled?: boolean
  readonly filters?: readonly TaskFilterValue[]
  readonly onRemoveFilter?: (id: string) => void
  readonly onClearFilters?: () => void
  readonly onOpenFilters?: () => void
  readonly filterButtonLabel?: string
  readonly clearButtonLabel?: string
  readonly locale?: TasksLocale
}

export type TaskFilterBarProps = TaskFilterBarCommonProps &
  (
    | {
        /** Controlled search value. A change handler is required with it. */
        readonly search: string
        readonly onSearchChange: (value: string) => void
        readonly defaultSearch?: never
      }
    | {
        /** Uncontrolled initial search value for isolated compositions. */
        readonly defaultSearch?: string
        readonly search?: never
        readonly onSearchChange?: never
      }
  )

export interface TaskDisplayOptions {
  readonly density: TasksDensity
  readonly visibleColumns?: Readonly<Record<string, boolean>>
}

interface DisplayMenuCommonProps {
  readonly options: TaskDisplayOptions
  readonly onDensityChange?: (density: TasksDensity) => void
  readonly onColumnVisibilityChange?: (columnId: string, visible: boolean) => void
  readonly columns?: readonly { readonly id: string; readonly label: string }[]
  readonly label?: string
}

export type DisplayMenuProps = DisplayMenuCommonProps &
  (
    | {
        /** Controlled open state. The owner must observe open changes. */
        readonly open: boolean
        readonly onOpenChange: (open: boolean) => void
        readonly defaultOpen?: never
      }
    | {
        /** Uncontrolled initial state for Storybook and lightweight compositions. */
        readonly defaultOpen?: boolean
        readonly open?: never
        readonly onOpenChange?: (open: boolean) => void
      }
  ) & { readonly locale?: TasksLocale }

export interface TaskStateBoundaryProps {
  readonly state: "loading" | "empty" | "offline" | "stale" | "recovering" | "error"
  readonly title?: string
  readonly detail?: string
  readonly actionLabel?: string
  readonly onAction?: () => void
  readonly children?: ReactNode
  readonly locale?: TasksLocale
}

export interface TaskCardProps {
  readonly task: BoardTaskViewModel
  readonly selected?: boolean
  readonly density?: TasksDensity
  readonly locale?: TasksLocale
  readonly statusLabel?: string
  readonly onSelect?: (task: BoardTaskViewModel) => void
}

export interface BoardColumnProps {
  readonly id: string
  readonly status: BoardTaskStatus
  readonly title: string
  readonly tasks: readonly BoardTaskViewModel[]
  readonly selectedTaskId?: string | null
  readonly density?: TasksDensity
  readonly locale?: TasksLocale
  readonly onSelectTask?: (task: BoardTaskViewModel) => void
  readonly emptyLabel?: string
}

export interface TaskTableProps {
  readonly tasks: readonly BoardTaskViewModel[]
  readonly selectedTaskId?: string | null
  readonly density?: TasksDensity
  readonly locale?: TasksLocale
  readonly visibleColumns?: Readonly<Record<string, boolean>>
  readonly onSelectTask?: (task: BoardTaskViewModel) => void
  readonly caption?: string
}

export interface SidePeekFrameProps {
  readonly model: TaskInspectorViewModel
  readonly locale?: TasksLocale
  /** `sheet` opts into dialog semantics and mobile focus handling. */
  readonly mode?: "side-peek" | "sheet"
  readonly onClose?: () => void
  /** The route/shell owns focus restoration; this is only a reusable seam. */
  readonly onRestoreFocus?: () => void
  readonly onOpenDetails?: (taskId: string) => void
  readonly closeLabel?: string
  readonly detailsLabel?: string
  readonly statusLabel?: string
  readonly requiredStepLabel?: string
  readonly runLabel?: string
}
