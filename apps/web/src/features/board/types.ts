/**
 * Board feature 的只读 presentation contract。
 *
 * 这里不暴露 generated response，也不持有 query/cache 规则；root integration
 * 应显式把已校验的 query value 映射到这些窄类型。
 */

export type BoardTaskStatus =
  | "triage"
  | "todo"
  | "scheduled"
  | "ready"
  | "running"
  | "blocked"
  | "review"
  | "done"
  | "archived"

export type BoardExecutionPlanState = "unplanned" | "planned" | "not_required"

export interface BoardIdentity {
  readonly id: string
  readonly slug: string
  readonly name: string
}

export interface BoardColumnViewModel {
  readonly id: string
  readonly status: BoardTaskStatus
  readonly title: string
  readonly position: number
  readonly hidden: boolean
}

export interface BoardTaskReadinessViewModel {
  readonly dependencyBlocked: boolean
  readonly unfinishedParentCount: number
  readonly executionPlanState: BoardExecutionPlanState
  readonly requiredStepCount: number
  readonly completedRequiredStepCount: number
  readonly optionalStepCount: number
}

export interface BoardTaskViewModel {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: BoardTaskStatus
  readonly position: number
  readonly priority: 0 | 1 | 2 | 3
  readonly assignee: string | null
  readonly readiness: BoardTaskReadinessViewModel
}

export interface BoardViewModel {
  readonly board: BoardIdentity
  readonly columns: readonly BoardColumnViewModel[]
  readonly tasksByStatus: Readonly<Record<string, readonly BoardTaskViewModel[]>>
}

export type BoardViewModelValidationResult =
  | { readonly valid: true }
  | { readonly valid: false; readonly message: string }

/**
 * 在 presentation seam 关闭会静默丢任务的模型。
 *
 * query adapter 通常会在更早阶段执行相同校验；组件再次 fail closed，避免
 * 将缺少 server column 的任务渲染成一个看似完整的 board。
 */
export function validateBoardViewModel(model: BoardViewModel): BoardViewModelValidationResult {
  const statuses = new Set<string>()
  for (const column of model.columns) {
    if (statuses.has(column.status)) {
      return { valid: false, message: `服务端返回重复状态列：${column.status}` }
    }
    statuses.add(column.status)
  }

  for (const [status, tasks] of Object.entries(model.tasksByStatus)) {
    if (tasks.length > 0 && !statuses.has(status)) {
      return { valid: false, message: `任务状态 ${status} 没有对应的服务端列` }
    }
    for (const task of tasks) {
      if (task.status !== status) {
        return { valid: false, message: `任务 ${task.ref} 的状态分组与任务事实不一致` }
      }
    }
  }

  return { valid: true }
}

export type BoardViewState =
  | { readonly kind: "loading" }
  | { readonly kind: "empty"; readonly board: BoardIdentity; readonly detail?: string }
  | { readonly kind: "error"; readonly message: string }
  | { readonly kind: "offline"; readonly message?: string }
  | { readonly kind: "ready"; readonly model: BoardViewModel }

export interface BoardMessages {
  readonly boardEyebrow: string
  readonly boardTitle: string
  readonly boardIdentityLabel: string
  readonly skipToColumns: string
  readonly columnNavigationLabel: string
  readonly columnTaskCount: (count: number) => string
  readonly emptyBoardTitle: string
  readonly emptyBoardDescription: string
  readonly emptyVisibleColumnsTitle: string
  readonly emptyVisibleColumnsDescription: string
  readonly emptyColumn: string
  readonly loading: string
  readonly errorTitle: string
  readonly offlineTitle: string
  readonly retry: string
  readonly statusLabel: string
  readonly priorityLabel: (priority: number) => string
  readonly assigneeLabel: string
  readonly unassigned: string
  readonly readinessLabel: string
  readonly dependencyLabel: string
  readonly dependencyBlocked: string
  readonly dependencyClear: string
  readonly planLabel: string
  readonly planState: Readonly<Record<BoardExecutionPlanState, string>>
  readonly requiredStepsLabel: string
  readonly optionalStepsLabel: string
}

export type BoardMessagesOverrides = Omit<Partial<BoardMessages>, "planState"> & {
  readonly planState?: Partial<Record<BoardExecutionPlanState, string>>
}

export const defaultBoardMessages: BoardMessages = {
  boardEyebrow: "ASTRYX BOARD",
  boardTitle: "看板",
  boardIdentityLabel: "看板标识",
  skipToColumns: "跳转到看板列",
  columnNavigationLabel: "看板列导航",
  columnTaskCount: (count) => `${count} 个任务`,
  emptyBoardTitle: "看板暂无列",
  emptyBoardDescription: "服务端还没有提供可见列。看板不会创建本地默认列。",
  emptyVisibleColumnsTitle: "看板没有可见列",
  emptyVisibleColumnsDescription: "服务端提供的列都标记为隐藏；看板不会创建本地默认列。",
  emptyColumn: "此列暂无任务。",
  loading: "正在加载看板…",
  errorTitle: "看板加载失败",
  offlineTitle: "当前处于离线状态",
  retry: "重试",
  statusLabel: "状态",
  priorityLabel: (priority) => `优先级 P${priority}`,
  assigneeLabel: "执行者",
  unassigned: "未分配",
  readinessLabel: "就绪性事实",
  dependencyLabel: "依赖",
  dependencyBlocked: "依赖阻塞",
  dependencyClear: "依赖已满足",
  planLabel: "执行计划",
  planState: {
    unplanned: "未规划",
    planned: "已规划",
    not_required: "无需计划",
  },
  requiredStepsLabel: "必需步骤",
  optionalStepsLabel: "可选步骤",
}
