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
  readonly lockVersion: number
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
  if (!model || !model.board) return { valid: false, message: "看板身份无效" }
  if (!Array.isArray(model.columns)) return { valid: false, message: "服务端列数据无效" }
  if (!model.tasksByStatus || typeof model.tasksByStatus !== "object") {
    return { valid: false, message: "任务状态分组无效" }
  }
  if (!hasText(model.board.id)) return { valid: false, message: "看板 id 不能为空" }
  if (!hasText(model.board.slug)) return { valid: false, message: "看板 slug 不能为空" }
  if (!hasText(model.board.name)) return { valid: false, message: "看板名称不能为空" }

  const columnIds = new Set<string>()
  const statuses = new Set<string>()
  const positions = new Set<number>()
  for (const column of model.columns) {
    if (!hasText(column.id)) return { valid: false, message: "服务端列 id 不能为空" }
    if (!hasText(column.title)) return { valid: false, message: "服务端列标题不能为空" }
    if (!Number.isSafeInteger(column.position)) {
      return { valid: false, message: `服务端列 ${column.id} 的 position 必须是 safe integer` }
    }
    if (columnIds.has(column.id)) return { valid: false, message: `服务端返回重复列 id：${column.id}` }
    if (positions.has(column.position)) return { valid: false, message: `服务端返回重复列 position：${column.position}` }
    if (statuses.has(column.status)) {
      return { valid: false, message: `服务端返回重复状态列：${column.status}` }
    }
    columnIds.add(column.id)
    statuses.add(column.status)
    positions.add(column.position)
  }

  const taskIds = new Set<string>()
  for (const [status, tasks] of Object.entries(model.tasksByStatus)) {
    if (!Array.isArray(tasks)) return { valid: false, message: `任务状态 ${status} 的任务分组无效` }
    if (tasks.length > 0 && !statuses.has(status)) {
      return { valid: false, message: `任务状态 ${status} 没有对应的服务端列` }
    }
    for (const task of tasks) {
      if (!hasText(task.id)) return { valid: false, message: "任务 id 不能为空" }
      if (!hasText(task.ref)) return { valid: false, message: `任务 ${task.id} 的 ref 不能为空` }
      if (!hasText(task.title)) return { valid: false, message: `任务 ${task.ref} 的标题不能为空` }
      if (!Number.isSafeInteger(task.position)) {
        return { valid: false, message: `任务 ${task.ref} 的 position 必须是 safe integer` }
      }
      if (taskIds.has(task.id)) return { valid: false, message: `服务端返回重复任务 id：${task.id}` }
      if (task.status !== status) {
        return { valid: false, message: `任务 ${task.ref} 的状态分组与任务事实不一致` }
      }
      taskIds.add(task.id)
    }
  }

  return { valid: true }
}

function hasText(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}

export type BoardViewState =
  | { readonly kind: "loading" }
  /**
   * Empty is also the explicit no-board state. The identity is optional so the
   * UI never has to invent a board just to render an empty response.
   */
  | { readonly kind: "empty"; readonly board?: BoardIdentity; readonly detail?: string }
  | { readonly kind: "error"; readonly message: string }
  | { readonly kind: "offline"; readonly message?: string }
  | { readonly kind: "ready"; readonly model: BoardViewModel }

export type BoardSyncStatus = "connecting" | "live" | "stale" | "recovering" | "circuit-open"

export interface BoardMessages {
  readonly boardEyebrow: string
  readonly boardTitle: string
  readonly boardIdentityLabel: string
  readonly boardColumnsLabel: string
  readonly skipToColumns: string
  readonly columnNavigationLabel: string
  readonly columnTaskCount: (count: number) => string
  readonly emptyBoardTitle: string
  readonly emptyBoardDescription: string
  readonly noBoardsTitle: string
  readonly noBoardsDescription: string
  readonly emptyVisibleColumnsTitle: string
  readonly emptyVisibleColumnsDescription: string
  readonly emptyColumn: string
  readonly loading: string
  readonly errorTitle: string
  readonly invalidModelDescription: string
  readonly offlineTitle: string
  readonly syncConnecting: string
  readonly syncLive: string
  readonly syncStale: string
  readonly syncRecovering: string
  readonly syncCircuitOpen: string
  readonly syncStaleDescription: string
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
  readonly createTask: string
  readonly editTask: string
  readonly grabTask: string
  readonly releaseTask: string
  readonly cancelGrab: string
  readonly dropTask: string
  readonly taskCardRoleDescription: string
  readonly transitionLabel: string
  readonly transitionNames: Readonly<Record<string, string>>
  readonly taskTitleLabel: string
  readonly createTaskTitle: string
  readonly editTaskTitle: string
  readonly transitionTaskTitle: string
  readonly blockReasonLabel: string
  readonly blockReasonPlaceholder: string
  readonly save: string
  readonly create: string
  readonly cancel: string
  readonly mutationPending: string
  readonly mutationError: string
  readonly conflictDescription: string
  readonly retryMutation: string
  readonly close: string
  readonly dropTargetLabel: (column: string) => string
}

export type BoardMessagesOverrides = Omit<Partial<BoardMessages>, "planState"> & {
  readonly planState?: Partial<Record<BoardExecutionPlanState, string>>
}

export const defaultBoardMessages: BoardMessages = {
  boardEyebrow: "ASTRYX BOARD",
  boardTitle: "看板",
  boardIdentityLabel: "看板标识",
  boardColumnsLabel: "看板列内容",
  skipToColumns: "跳转到看板列",
  columnNavigationLabel: "看板列导航",
  columnTaskCount: (count) => `${count} 个任务`,
  emptyBoardTitle: "看板暂无列",
  emptyBoardDescription: "服务端还没有提供可见列。看板不会创建本地默认列。",
  noBoardsTitle: "暂无看板",
  noBoardsDescription: "服务端没有返回可用看板；看板不会创建本地默认看板。",
  emptyVisibleColumnsTitle: "看板没有可见列",
  emptyVisibleColumnsDescription: "服务端提供的列都标记为隐藏；看板不会创建本地默认列。",
  emptyColumn: "此列暂无任务。",
  loading: "正在加载看板…",
  errorTitle: "看板加载失败",
  invalidModelDescription: "服务端返回的看板数据暂时无法显示，请重试。",
  offlineTitle: "当前处于离线状态",
  syncConnecting: "正在连接实时同步…",
  syncLive: "实时同步已连接",
  syncStale: "同步暂时中断",
  syncRecovering: "正在恢复同步",
  syncCircuitOpen: "同步暂时不可用",
  syncStaleDescription: "仍显示最近一次成功读取的看板数据。",
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
  createTask: "新建任务",
  editTask: "编辑任务",
  grabTask: "抓取任务",
  releaseTask: "放下任务",
  cancelGrab: "取消抓取",
  dropTask: "移动任务",
  taskCardRoleDescription: "可拖动任务卡片",
  transitionLabel: "允许的状态转换",
  transitionNames: {
    specify: "补充规格",
    promote: "推进就绪",
    claim: "开始执行",
    heartbeat: "保持执行",
    complete: "标记完成",
    "submit-review": "提交审查",
    block: "标记阻塞",
    unblock: "解除阻塞",
    archive: "归档任务",
  },
  taskTitleLabel: "任务标题",
  createTaskTitle: "新建任务",
  editTaskTitle: "编辑任务",
  transitionTaskTitle: "状态转换",
  blockReasonLabel: "阻塞原因",
  blockReasonPlaceholder: "说明为什么暂时无法继续",
  save: "保存",
  create: "创建",
  cancel: "取消",
  mutationPending: "正在保存…",
  mutationError: "任务操作失败",
  conflictDescription: "任务已被其他操作更新。已重新读取 canonical 状态，请确认输入后重试。",
  retryMutation: "重新尝试",
  close: "关闭",
  dropTargetLabel: (column) => `放置到${column}`,
}

export const englishBoardMessages: BoardMessages = {
  boardEyebrow: "ASTRYX BOARD",
  boardTitle: "Board",
  boardIdentityLabel: "Board identity",
  boardColumnsLabel: "Board columns",
  skipToColumns: "Skip to board columns",
  columnNavigationLabel: "Board column navigation",
  columnTaskCount: (count) => `${count} tasks`,
  emptyBoardTitle: "This board has no columns",
  emptyBoardDescription: "The server has not provided any visible columns. No local default columns are created.",
  noBoardsTitle: "No boards available",
  noBoardsDescription: "The server returned no available boards. No local default board is created.",
  emptyVisibleColumnsTitle: "No visible columns",
  emptyVisibleColumnsDescription: "All server-provided columns are hidden. No local default columns are created.",
  emptyColumn: "No tasks in this column.",
  loading: "Loading board…",
  errorTitle: "Board could not be loaded",
  invalidModelDescription: "The server returned board data that cannot be displayed. Try again.",
  offlineTitle: "You are offline",
  syncConnecting: "Connecting to live sync…",
  syncLive: "Live sync connected",
  syncStale: "Sync is temporarily interrupted",
  syncRecovering: "Recovering sync",
  syncCircuitOpen: "Sync is temporarily unavailable",
  syncStaleDescription: "The most recently loaded board data is still displayed.",
  retry: "Retry",
  statusLabel: "Status",
  priorityLabel: (priority) => `Priority P${priority}`,
  assigneeLabel: "Assignee",
  unassigned: "Unassigned",
  readinessLabel: "Readiness facts",
  dependencyLabel: "Dependencies",
  dependencyBlocked: "Blocked by dependencies",
  dependencyClear: "Dependencies clear",
  planLabel: "Execution plan",
  planState: {
    unplanned: "Unplanned",
    planned: "Planned",
    not_required: "Not required",
  },
  requiredStepsLabel: "Required steps",
  optionalStepsLabel: "Optional steps",
  createTask: "Create task",
  editTask: "Edit task",
  grabTask: "Grab task",
  releaseTask: "Drop task",
  cancelGrab: "Cancel grab",
  dropTask: "Move task",
  taskCardRoleDescription: "Draggable task card",
  transitionLabel: "Allowed status transitions",
  transitionNames: {
    specify: "Specify task",
    promote: "Promote to ready",
    claim: "Start execution",
    heartbeat: "Keep running",
    complete: "Mark done",
    "submit-review": "Submit for review",
    block: "Block task",
    unblock: "Unblock task",
    archive: "Archive task",
  },
  taskTitleLabel: "Task title",
  createTaskTitle: "Create task",
  editTaskTitle: "Edit task",
  transitionTaskTitle: "Status transition",
  blockReasonLabel: "Block reason",
  blockReasonPlaceholder: "Explain why work cannot continue",
  save: "Save",
  create: "Create",
  cancel: "Cancel",
  mutationPending: "Saving…",
  mutationError: "Task action failed",
  conflictDescription: "This task changed elsewhere. Canonical state was reloaded; review your input and try again.",
  retryMutation: "Try again",
  close: "Close",
  dropTargetLabel: (column) => `Drop in ${column}`,
}

export function boardMessagesForLocale(locale: "zh" | "en"): BoardMessages {
  return locale === "en" ? englishBoardMessages : defaultBoardMessages
}
