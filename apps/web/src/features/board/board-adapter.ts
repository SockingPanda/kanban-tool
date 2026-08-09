import type {
  BoardReadModel,
  BoardTask,
} from "../../lib/api/board-read-model"
import type {
  BoardColumnViewModel,
  BoardExecutionPlanState,
  BoardTaskStatus,
  BoardTaskViewModel,
  BoardViewModel,
} from "./types"

/** Errors at the read-model/presentation boundary are intentionally typed. */
export class BoardAdapterError extends Error {
  readonly kind = "invalid-board-view-model-contract" as const

  constructor(message: string) {
    super(message)
    this.name = "BoardAdapterError"
  }
}

/**
 * Map the generated read contract to the small presentation contract.
 *
 * The switch statements are deliberate: a newly added wire enum must be
 * handled here before it can reach the board UI. The adapter does not sort or
 * infer state-machine transitions; it only projects already validated facts.
 */
export function toBoardViewModel(readModel: BoardReadModel): BoardViewModel {
  const columns = Object.freeze(readModel.columns.map((column) => Object.freeze({
    id: column.id,
    status: parseStatus(column.status),
    title: column.title,
    position: column.position,
    hidden: column.hidden,
  } satisfies BoardColumnViewModel)))

  const grouped: Record<string, readonly BoardTaskViewModel[]> = {}
  for (const [rawStatus, tasks] of Object.entries(readModel.tasksByStatus)) {
    if (tasks === undefined) continue
    const status = parseStatus(rawStatus)
    const projected = tasks.map((task) => projectTask(task, status))
    grouped[status] = Object.freeze(projected)
  }

  return Object.freeze({
    board: Object.freeze({
      id: readModel.identity.canonicalBoardId,
      slug: readModel.identity.slug,
      name: readModel.identity.name,
    }),
    columns,
    tasksByStatus: Object.freeze(grouped),
  })
}

function projectTask(task: BoardTask, groupedStatus: BoardTaskStatus): BoardTaskViewModel {
  const status = parseStatus(task.status)
  if (status !== groupedStatus) {
    throw new BoardAdapterError(`任务 ${task.ref} 的状态分组与任务事实不一致。`)
  }
  return Object.freeze({
    id: task.id,
    ref: task.ref,
    title: task.title,
    status,
    position: task.position,
    lockVersion: task.lock_version,
    priority: parsePriority(task.priority),
    assignee: task.assignee,
    readiness: Object.freeze({
      dependencyBlocked: task.dependency_blocked,
      unfinishedParentCount: task.unfinished_parent_count,
      executionPlanState: parseExecutionPlanState(task.execution_plan_state),
      requiredStepCount: task.required_step_count,
      completedRequiredStepCount: task.completed_required_step_count,
      optionalStepCount: task.optional_step_count,
    }),
  })
}

function parseStatus(value: string): BoardTaskStatus {
  switch (value) {
    case "triage":
    case "todo":
    case "scheduled":
    case "ready":
    case "running":
    case "blocked":
    case "review":
    case "done":
    case "archived":
      return value
    default:
      throw new BoardAdapterError(`服务端返回未知任务状态：${JSON.stringify(value)}。`)
  }
}

function parsePriority(value: number): 0 | 1 | 2 | 3 {
  switch (value) {
    case 0:
    case 1:
    case 2:
    case 3:
      return value
    default:
      throw new BoardAdapterError(`服务端返回无效任务优先级：${JSON.stringify(value)}。`)
  }
}

function parseExecutionPlanState(value: string): BoardExecutionPlanState {
  switch (value) {
    case "unplanned":
    case "planned":
    case "not_required":
      return value
    default:
      throw new BoardAdapterError(`服务端返回未知执行计划状态：${JSON.stringify(value)}。`)
  }
}
