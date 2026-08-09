import type {
  InspectorCommentInput,
  InspectorCreateStepInput,
} from "./task-inspector-mutation-state"

export type TaskInspectorCommentFormInput = InspectorCommentInput & { readonly author: string }

export function buildCommentInput(author: string, kind: string, body: string): TaskInspectorCommentFormInput {
  return { author: author.trim(), kind: kind.trim() as "note" | "decision" | "signal", body: body.trim() }
}

export function buildStepInput(title: string, body: string, required: boolean, linkedTaskRef?: string): InspectorCreateStepInput {
  const input: InspectorCreateStepInput = { title: title.trim(), required }
  const trimmedBody = body.trim()
  const trimmedLink = linkedTaskRef?.trim()
  if (trimmedBody) Object.assign(input, { body: trimmedBody })
  if (trimmedLink) Object.assign(input, { linked_task_ref: trimmedLink })
  return input
}

export function buildPlanInput(reason: string): { readonly reason: string } {
  return { reason: reason.trim() }
}

/** Pure builders are exported for direct seam tests without coupling tests to React internals. */
export const __test = {
  commentInput: buildCommentInput,
  stepInput: buildStepInput,
  planInput: buildPlanInput,
}
