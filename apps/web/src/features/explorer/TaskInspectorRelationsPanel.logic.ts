import type {
  InspectorCreateStepInput,
} from "./task-inspector-mutation-state"

export function buildCommentInput(kind: string, body: string) {
  return { kind: kind.trim() as "note" | "decision" | "signal", body: body.trim() }
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

export type TaskSelectorResolver = (selector: string) => string | null

/** Resolve a user-entered canonical task id or same-board ref before a mutation. */
export function resolveTaskSelector(selector: string, resolver: TaskSelectorResolver): string | null {
  const trimmed = selector.trim()
  if (/^t_\S+$/.test(trimmed)) return trimmed
  try {
    const resolved = resolver(trimmed)
    return typeof resolved === "string" && /^t_\S+$/.test(resolved.trim()) ? resolved.trim() : null
  } catch {
    return null
  }
}

/** Pure builders are exported for direct seam tests without coupling tests to React internals. */
export const __test = {
  commentInput: buildCommentInput,
  stepInput: buildStepInput,
  planInput: buildPlanInput,
  resolveTaskSelector,
}
