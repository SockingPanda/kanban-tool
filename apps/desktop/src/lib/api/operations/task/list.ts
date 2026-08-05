import { ApiTransport } from "../../transport"
import { parseTaskListEnvelope } from "./parsers"
import type { TaskListOptions, TaskPageResult } from "../../types"

export function taskListParams(options: TaskListOptions = {}) {
  const params = new URLSearchParams()
  const limit = options.limit ?? 100
  const offset = options.offset ?? 0
  params.set("include_archived", String(options.includeArchived ?? false))
  params.set("limit", String(limit))
  params.set("offset", String(offset))
  params.set("sort", options.sort ?? "-updated_at")
  if (options.query?.trim()) params.set("q", options.query.trim())
  for (const status of options.statuses ?? []) params.append("status", status)
  for (const priority of options.priorities ?? []) params.append("priority", String(priority))
  for (const label of options.labels ?? []) { if (label.trim()) params.append("label", label.trim()) }
  for (const filter of options.planFilters ?? []) params.append("plan_filter", filter)
  return params
}
export async function listTasks(api: ApiTransport, options: TaskListOptions = {}) {
    const params = taskListParams(options)
    const envelope = parseTaskListEnvelope(await api.requestRaw(`/api/v1/boards/${api.board}/tasks?${params.toString()}`, { signal: options.signal }))
    return { tasks: envelope.data, page: envelope.meta } satisfies TaskPageResult
  }
