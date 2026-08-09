import type {
  InspectorCreateStepInput,
} from "./task-inspector-mutation-state"

export type CommentSortOrder = "newest" | "oldest"

export interface CommentPageItem {
  readonly id: string
  readonly createdAt: number
}

export interface CommentPageResult<T extends CommentPageItem> {
  readonly comments: readonly T[]
  readonly page: number
  readonly pageCount: number
  readonly hasPreviousPage: boolean
  readonly hasNextPage: boolean
}

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

export type StepSubmitMode = "create" | "link"

export interface StepSubmission {
  readonly operation: "createStep" | "linkStep"
  readonly input: InspectorCreateStepInput
}

/** Build the exact command selected by a form action; create never inherits a link field. */
export function buildStepSubmission(title: string, body: string, required: boolean, linkedTaskRef: string | undefined, mode: StepSubmitMode): StepSubmission | null {
  const input = buildStepInput(title, body, required, mode === "link" ? linkedTaskRef : undefined)
  if (!input.title || (mode === "link" && !input.linked_task_ref)) return null
  return { operation: mode === "link" ? "linkStep" : "createStep", input }
}

export function buildPlanInput(reason: string): { readonly reason: string } {
  return { reason: reason.trim() }
}

export function sortedComments<T extends CommentPageItem>(comments: readonly T[], sortOrder: CommentSortOrder): T[] {
  return [...comments].sort((left, right) => {
    const createdDiff = left.createdAt - right.createdAt
    const idDiff = left.id.localeCompare(right.id)
    const diff = createdDiff || idDiff
    return sortOrder === "newest" ? -diff : diff
  })
}

export function commentPageState<T extends CommentPageItem>(comments: readonly T[], page: number, pageSize: number, sortOrder: CommentSortOrder): CommentPageResult<T> {
  const safePageSize = Number.isSafeInteger(pageSize) && pageSize > 0 ? pageSize : 10
  const sorted = sortedComments(comments, sortOrder)
  const pageCount = Math.max(1, Math.ceil(sorted.length / safePageSize))
  const currentPage = Math.min(Math.max(page, 0), pageCount - 1)
  return {
    comments: sorted.slice(currentPage * safePageSize, currentPage * safePageSize + safePageSize),
    page: currentPage,
    pageCount,
    hasPreviousPage: currentPage > 0,
    hasNextPage: currentPage < pageCount - 1,
  }
}

export function formatCommentDateTime(value: number, locale: "zh" | "en"): { readonly label: string; readonly iso: string } {
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return { label: "—", iso: "" }
  const formatter = new Intl.DateTimeFormat(locale === "zh" ? "zh-CN" : "en-US", { dateStyle: "medium", timeStyle: "short" })
  return { label: formatter.format(date), iso: date.toISOString() }
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
  commentPageState,
  formatCommentDateTime,
  stepSubmission: buildStepSubmission,
  stepInput: buildStepInput,
  planInput: buildPlanInput,
  resolveTaskSelector,
}
