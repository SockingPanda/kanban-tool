import { ApiError, expectArray, expectRecord, expectExactKeys, expectString, expectBoolean, expectSafeInteger, expectNullableString, expectNullableInteger } from "../../parsers"
import type { StepPlanState, StepStatus, TaskExecutionPlan, TaskStep, TaskSteps } from "../../types"
import { parseApiTask } from "../task/parsers"

const STEP_KEYS = ["id", "parent_task_id", "title", "body", "linked_task", "position", "required", "status", "resolution_note", "resolved_by", "resolved_at", "created_by", "created_at", "updated_by", "updated_at"] as const
const EXECUTION_PLAN_KEYS = ["board_id", "task_id", "state", "reason", "updated_by", "updated_at"] as const
const STEP_STATUSES = new Set<StepStatus>(["todo", "done", "skipped"])
const PLAN_STATES = new Set<StepPlanState>(["unplanned", "planned", "not_required"])

export function parseTaskStep(value: unknown, label: string): TaskStep {
  const step = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(step, STEP_KEYS, label)
  for (const key of ["id", "parent_task_id", "title", "created_by", "updated_by"] as const) expectString(step[key], `${label}.${key}`)
  for (const key of ["body", "resolution_note", "resolved_by"] as const) expectNullableString(step[key], `${label}.${key}`)
  for (const key of ["position", "created_at", "updated_at"] as const) expectSafeInteger(step[key], `${label}.${key}`)
  expectNullableInteger(step.resolved_at, `${label}.resolved_at`); expectBoolean(step.required, `${label}.required`)
  if (!STEP_STATUSES.has(step.status as StepStatus)) throw new ApiError("invalid_response", `${label}.status is unknown`)
  if (step.linked_task !== null) step.linked_task = parseApiTask(step.linked_task, `${label}.linked_task`)
  return step as TaskStep
}

export function parseExecutionPlan(value: unknown, label: string): TaskExecutionPlan {
  const plan = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(plan, EXECUTION_PLAN_KEYS, label)
  for (const key of ["board_id", "task_id", "updated_by"] as const) expectString(plan[key], `${label}.${key}`)
  expectNullableString(plan.reason, `${label}.reason`); expectSafeInteger(plan.updated_at, `${label}.updated_at`)
  if (!PLAN_STATES.has(plan.state as StepPlanState)) throw new ApiError("invalid_response", `${label}.state is unknown`)
  return plan as TaskExecutionPlan
}

export function parseStepsEnvelope(value: unknown, label: string): { data: TaskSteps } {
  const envelope = expectRecord<Record<string, unknown>>(value, label); expectExactKeys(envelope, ["data"], label)
  const data = expectRecord<Record<string, unknown>>(envelope.data, `${label}.data`); expectExactKeys(data, ["task_id", "steps", "execution_plan"], `${label}.data`)
  return { data: { task_id: expectString(data.task_id, `${label}.data.task_id`), steps: expectArray<unknown>(data.steps, `${label}.data.steps`).map((step, index) => parseTaskStep(step, `${label}.data.steps[${index}]`)), execution_plan: parseExecutionPlan(data.execution_plan, `${label}.data.execution_plan`) } }
}
export function parseListStepsEnvelope(value: unknown) { return parseStepsEnvelope(value, "list steps response") }
export function parseCreateStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "create step response") }
export function parseUpdateStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "update step response") }
export function parseRemoveStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "remove step response") }
export function parseCompleteStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "complete step response") }
export function parseSkipStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "skip step response") }
export function parseReopenStepEnvelope(value: unknown) { return parseStepsEnvelope(value, "reopen step response") }
