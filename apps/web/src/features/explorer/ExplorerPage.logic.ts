import { ExplorerReadError } from "../../lib/api/explorer-read-model"
import type { TaskInspectorReadModel } from "../../lib/api/explorer-read-model"
import type { BoardRouteView } from "../../lib/router"
import type {
  TaskInspectorCommentView,
  TaskInspectorDependenciesView,
  TaskInspectorRelationTaskView,
  TaskInspectorStepView,
} from "./TaskInspectorRelationsPanel"

export type AsyncReadState<T> = {
  readonly data: T | null
  readonly error: ExplorerReadError | Error | null
  readonly loading: boolean
}

export type AsyncReadToken = {
  readonly identityKey: string
  readonly requestKey: string
}

export type AsyncReadInternalState<T> = AsyncReadState<T> & AsyncReadToken

export function asyncReadToken(enabled: boolean, key: string, generation: number): AsyncReadToken {
  const identityKey = `${enabled ? "enabled" : "disabled"}|${key}`
  return { identityKey, requestKey: `${identityKey}|${generation}` }
}

export function visibleAsyncReadState<T>(
  state: AsyncReadInternalState<T>,
  token: AsyncReadToken,
  enabled: boolean,
): AsyncReadState<T> {
  const sameIdentity = state.identityKey === token.identityKey
  const currentRequest = sameIdentity && state.requestKey === token.requestKey
  return {
    data: sameIdentity ? state.data : null,
    error: currentRequest ? state.error : null,
    loading: sameIdentity ? (currentRequest ? state.loading : true) : enabled,
  }
}

export function shouldClearMapTaskFromInspector(
  view: BoardRouteView,
  taskId: string | null,
  error: ExplorerReadError | Error | null,
): boolean {
  return view === "map" && Boolean(taskId) && error instanceof ExplorerReadError && error.reason === "task-not-found"
}

/** Keep the first canonical relation row for each id and preserve read-model order. */
export function firstById<T extends { readonly id: string }>(items: readonly T[]): T[] {
  const seen = new Set<string>()
  return items.filter((item) => {
    if (seen.has(item.id)) return false
    seen.add(item.id)
    return true
  })
}

type InspectorRelationTaskRow = Pick<TaskInspectorReadModel["dependencies"]["parents"][number], "id" | "ref" | "title" | "status">
type InspectorCommentRow = Pick<TaskInspectorReadModel["comments"][number], "id" | "author" | "kind" | "body" | "created_at" | "metadata">
type InspectorStepRow = Pick<TaskInspectorReadModel["steps"]["steps"][number], "id" | "title" | "body" | "required" | "status" | "linked_task">
type InspectorExecutionPlanRow = Pick<TaskInspectorReadModel["steps"]["execution_plan"], "state" | "reason">

function relationTaskView(task: InspectorRelationTaskRow): TaskInspectorRelationTaskView {
  return { id: task.id, ref: task.ref, title: task.title, status: task.status }
}

function linkedTaskView(task: NonNullable<InspectorStepRow["linked_task"]>): TaskInspectorRelationTaskView {
  return { id: task.id, ref: task.ref, title: task.title, status: task.status }
}

/** Adapter boundary for the mutation relation surface; duplicate ids never become duplicate intents or React keys. */
export interface InspectorRelationsReadModel {
  readonly comments: readonly InspectorCommentRow[]
  readonly dependencies: {
    readonly parents: readonly InspectorRelationTaskRow[]
    readonly children: readonly InspectorRelationTaskRow[]
  }
  readonly steps: {
    readonly steps: readonly InspectorStepRow[]
    readonly execution_plan: InspectorExecutionPlanRow
  }
}

export function inspectorRelationsView(model: InspectorRelationsReadModel): {
  readonly comments: readonly TaskInspectorCommentView[]
  readonly dependencies: TaskInspectorDependenciesView
  readonly steps: { readonly steps: readonly TaskInspectorStepView[]; readonly executionPlan: { readonly state: "unplanned" | "planned" | "not_required"; readonly reason: string | null } }
} {
  return {
    comments: firstById(model.comments).map((comment) => ({
      id: comment.id,
      author: comment.author,
      kind: comment.kind,
      body: comment.body,
      createdAt: comment.created_at,
      metadata: comment.metadata,
    })),
    dependencies: {
      parents: firstById(model.dependencies.parents).map(relationTaskView),
      children: firstById(model.dependencies.children).map(relationTaskView),
    },
    steps: {
      steps: firstById(model.steps.steps).map((step) => ({
        id: step.id,
        title: step.title,
        body: step.body,
        required: step.required,
        status: step.status,
        linkedTask: step.linked_task ? linkedTaskView(step.linked_task) : null,
      })),
      executionPlan: {
        state: model.steps.execution_plan.state,
        reason: model.steps.execution_plan.reason,
      },
    },
  }
}
