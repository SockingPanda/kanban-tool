import type { ExplorerReadError, TaskListPlanFilter, TaskListQueryState, TaskListStatus } from '../../application/data/explorer-read-model';
import type { Locale } from '../../platform/preferences/preferences';
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
  readonly onCreate?: (trigger?: HTMLElement | null) => void
  readonly isMutationPending?: boolean
  readonly locale?: Locale
}

export type ListCopy = {
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
  readonly plan: string
  readonly reset: string
  readonly error: string
  readonly offline: string
  readonly retry: string
  readonly empty: string
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
