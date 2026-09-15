export type TaskInspectorRelationTaskStatus =
  | "triage"
  | "todo"
  | "scheduled"
  | "ready"
  | "running"
  | "blocked"
  | "review"
  | "done"
  | "archived"

export type TaskInspectorCommentKind = "note" | "decision" | "signal"

export interface TaskInspectorCommentView {
  readonly id: string
  readonly author: string
  readonly kind: TaskInspectorCommentKind
  readonly body: string
  readonly createdAt: number
  /** Structured metadata is rendered as escaped JSON; it is never interpreted as HTML. */
  readonly metadata?: Readonly<Record<string, unknown>>
}

export interface TaskInspectorRelationTaskView {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: TaskInspectorRelationTaskStatus
}

export interface TaskInspectorDependenciesView {
  readonly parents: readonly TaskInspectorRelationTaskView[]
  readonly children: readonly TaskInspectorRelationTaskView[]
}

export type TaskInspectorLinkedTaskView = TaskInspectorRelationTaskView

export interface TaskInspectorStepView {
  readonly id: string
  readonly title: string
  readonly body: string | null
  readonly required: boolean
  readonly status: "todo" | "done" | "skipped"
  readonly linkedTask?: TaskInspectorLinkedTaskView | null
}

export interface TaskInspectorExecutionPlanView {
  readonly state: "unplanned" | "planned" | "not_required"
  readonly reason?: string | null
}

export interface TaskInspectorStepsView {
  readonly steps: readonly TaskInspectorStepView[]
  readonly executionPlan?: TaskInspectorExecutionPlanView
}

export type TaskInspectorRelationsStepsInput = TaskInspectorStepsView | readonly TaskInspectorStepView[]
