/**
 * Canonical URL vocabulary shared by the Tasks workspace and its Inspector.
 *
 * The URL is a projection of the current view, not a bag of state owned by
 * every projection. Keep the shared task/search selectors explicit so a view
 * transition cannot accidentally carry another view's controls along.
 */

export type TasksViewQuery = "board" | "list" | "table" | "map"
export type TasksRouteQuery = TasksViewQuery | "runs" | "events"

export type TaskSelectorMalformedReason = "empty" | "duplicate" | "unsafe" | "non_canonical"

export type TaskSelectorState =
  | { readonly kind: "missing"; readonly value: null }
  | { readonly kind: "valid"; readonly value: string }
  | { readonly kind: "malformed"; readonly value: string; readonly reason: TaskSelectorMalformedReason }

type QueryInput = string | URLSearchParams

const sharedQueryKeys = new Set(["q", "task"])
const viewQueryKeys: Readonly<Record<TasksRouteQuery, ReadonlySet<string>>> = {
  board: new Set(["status"]),
  list: new Set(["status", "priority", "plan", "sort", "page", "limit", "include_archived"]),
  table: new Set(["status", "priority", "plan", "sort", "page", "limit", "include_archived"]),
  map: new Set(["filter", "show_done", "hide_isolated", "zoom"]),
  runs: new Set(),
  events: new Set(["kind"]),
}

function queryParams(input: QueryInput | null | undefined): URLSearchParams {
  if (input instanceof URLSearchParams) return new URLSearchParams(input)
  if (input === null || input === undefined) return new URLSearchParams()
  return new URLSearchParams(input.startsWith("?") ? input.slice(1) : input)
}

function hasUnsafeTaskSelector(value: string): boolean {
  if (value.trim() !== value || value.length === 0 || /[\\/?#]/u.test(value)) return true
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    if (codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)) return true
  }
  return false
}

function parseTaskSelectorValue(value: string, duplicate = false): TaskSelectorState {
  if (duplicate) return { kind: "malformed", value, reason: "duplicate" }
  if (value.length === 0) return { kind: "malformed", value, reason: "empty" }
  if (hasUnsafeTaskSelector(value)) return { kind: "malformed", value, reason: "unsafe" }
  if (!value.startsWith("t_") || value.length <= 2) return { kind: "malformed", value, reason: "non_canonical" }
  return { kind: "valid", value }
}

/**
 * Parse either a raw canonical task selector or a URL query containing
 * `task`. A present-but-invalid query is deliberately not treated as absent:
 * callers can render an explicit malformed-link state and skip all reads.
 */
export function parseTaskSelector(input: QueryInput | string | null | undefined): TaskSelectorState {
  if (input === null || input === undefined) return { kind: "missing", value: null }
  if (input instanceof URLSearchParams) {
    const values = input.getAll("task")
    if (values.length === 0) return { kind: "missing", value: null }
    return parseTaskSelectorValue(values[0] ?? "", values.length !== 1)
  }

  // A bare value is useful at the API/read-model boundary; query strings are
  // identified by the syntax URLSearchParams accepts.
  if (!/[=&]/u.test(input) && !input.startsWith("?")) {
    return input.length === 0 ? { kind: "missing", value: null } : parseTaskSelectorValue(input)
  }
  const params = queryParams(input)
  const values = params.getAll("task")
  if (values.length === 0) return { kind: "missing", value: null }
  return parseTaskSelectorValue(values[0] ?? "", values.length !== 1)
}

/** Explicit alias for callers that parse a raw API/path selector. */
export const parseCanonicalTaskSelector = parseTaskSelector

/** Return the canonical selector or null without changing the malformed state. */
export function canonicalTaskSelector(input: string | null | undefined): string | null {
  const parsed = parseTaskSelector(input)
  return parsed.kind === "valid" ? parsed.value : null
}

export function normalizeTaskSearch(value: string | null | undefined): string {
  if (value === null || value === undefined || value.includes("\u0000")) return ""
  const trimmed = value.trim()
  return trimmed.length <= 1024 ? trimmed : trimmed.slice(0, 1024)
}

/** Parse shared `q` and `task` fields without applying a view's filters. */
export function parseTasksUrl(input: QueryInput): {
  readonly query: URLSearchParams
  readonly search: string
  readonly task: TaskSelectorState
} {
  const query = queryParams(input)
  return {
    query,
    search: normalizeTaskSearch(query.get("q")),
    task: parseTaskSelector(query),
  }
}

/**
 * Keep shared fields and only the query fields owned by the target view.
 * Existing parameter order is retained for stable deep links; a requested
 * table display is appended after the retained fields.
 */
export function queryForTasksView(input: QueryInput, view: TasksRouteQuery, display?: "grouped" | "table"): URLSearchParams {
  const source = queryParams(input)
  const owned = new Set([...sharedQueryKeys, ...viewQueryKeys[view]])
  const next = new URLSearchParams()
  for (const [key, value] of source) {
    if (key === "display" || owned.has(key)) {
      if (key === "display") continue
      next.append(key, value)
    }
  }

  const listLike = view === "list" || view === "table"
  const table = listLike && (view === "table" || display === "table" || (display === undefined && source.get("display") === "table"))
  if (table) next.set("display", "table")
  return next
}

/** Alias with an imperative name for route/navigation callers. */
export const retainTasksViewQuery = queryForTasksView
