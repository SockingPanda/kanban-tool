import type { ReactNode } from "react"

import type { BoardTaskStatus, BoardTaskViewModel } from "../../features/board/types"
import type { TaskInspectorViewModel } from "../../features/explorer/TaskInspector"

export type TasksView = "board" | "list" | "table" | "map" | "timeline"

export type TasksDensity = "dense" | "comfortable"

export interface TaskFilterValue {
  readonly id: string
  readonly label: string
  readonly removable?: boolean
}

export interface TaskFilterBarProps {
  readonly search?: string
  readonly placeholder?: string
  readonly filters?: readonly TaskFilterValue[]
  readonly onSearchChange?: (value: string) => void
  readonly onRemoveFilter?: (id: string) => void
  readonly onClearFilters?: () => void
  readonly onOpenFilters?: () => void
  readonly filterButtonLabel?: string
  readonly clearButtonLabel?: string
}

export interface TaskDisplayOptions {
  readonly density: TasksDensity
  readonly visibleColumns?: Readonly<Record<string, boolean>>
}

export interface DisplayMenuProps {
  readonly options: TaskDisplayOptions
  /** Uncontrolled initial state for Storybook and lightweight compositions. */
  readonly defaultOpen?: boolean
  /** Controlled open state for shells that own popover state. */
  readonly open?: boolean
  readonly onOpenChange?: (open: boolean) => void
  readonly onDensityChange?: (density: TasksDensity) => void
  readonly onColumnVisibilityChange?: (columnId: string, visible: boolean) => void
  readonly columns?: readonly { readonly id: string; readonly label: string }[]
  readonly label?: string
}

export interface TaskStateBoundaryProps {
  readonly state: "loading" | "empty" | "offline" | "stale" | "recovering" | "error"
  readonly title?: string
  readonly detail?: string
  readonly actionLabel?: string
  readonly onAction?: () => void
  readonly children?: ReactNode
}

export interface TaskCardProps {
  readonly task: BoardTaskViewModel
  readonly selected?: boolean
  readonly density?: TasksDensity
  readonly locale?: "zh" | "en"
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
  readonly onSelectTask?: (task: BoardTaskViewModel) => void
  readonly emptyLabel?: string
}

export interface TaskTableProps {
  readonly tasks: readonly BoardTaskViewModel[]
  readonly selectedTaskId?: string | null
  readonly density?: TasksDensity
  readonly locale?: "zh" | "en"
  readonly onSelectTask?: (task: BoardTaskViewModel) => void
  readonly caption?: string
}

export interface SidePeekFrameProps {
  readonly model: TaskInspectorViewModel
  readonly locale?: "zh" | "en"
  readonly onClose?: () => void
  readonly onOpenDetails?: (taskId: string) => void
  readonly closeLabel?: string
  readonly detailsLabel?: string
  readonly statusLabel?: string
  readonly requiredStepLabel?: string
  readonly runLabel?: string
}
