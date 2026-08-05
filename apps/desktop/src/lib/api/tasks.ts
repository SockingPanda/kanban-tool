import { ApiTransport } from "./transport"
import { expectArray, expectRecord, normalizePageMeta } from "./parsers"
import type { PageEnvelopeMeta, SearchTaskHit, SearchTaskOptions, SearchTaskStatusWindowResponse, SearchTaskStatusWindowsResponse, SearchTasksResponse, SearchTasksResult, SearchTaskStatusWindowsResult, Task, TaskStatus } from "./types"

export { listTasks } from "./operations/task/list"
export { listTasksByStatus } from "./operations/task/list-by-status"
export { createTask } from "./operations/task/create"
export { updateTask } from "./operations/task/update"
export { getTask } from "./operations/task/show"
export { listDependencies } from "./operations/task/dependencies-list"
export { addDependency } from "./operations/task/dependencies-add"
export { removeDependency } from "./operations/task/dependencies-remove"
export { getTaskNeighborhood } from "./operations/task/neighborhood"
export { getBoardTaskMap } from "./operations/task/map"
export { markExecutionPlanNotRequired } from "./operations/task/plan-not-required"

function searchTaskParams(api: ApiTransport, options: SearchTaskOptions) {
  const params = new URLSearchParams()
  const limit = options.limit ?? 100
  const offset = options.offset ?? 0
  params.set("board", api.board)
  params.set("q", options.query.trim())
  params.set("include_archived", String(options.includeArchived ?? false))
  params.set("limit", String(limit))
  params.set("offset", String(offset))
  for (const status of options.statuses ?? []) params.append("status", status)
  for (const label of options.labels ?? []) { if (label.trim()) params.append("label", label.trim()) }
  return params
}
export async function searchTasks(api: ApiTransport, options: SearchTaskOptions) {
    const params = new URLSearchParams()
    const limit = options.limit ?? 100
    const offset = options.offset ?? 0
    params.set("board", api.board)
    params.set("q", options.query.trim())
    params.set("include_archived", String(options.includeArchived ?? false))
    params.set("limit", String(limit))
    params.set("offset", String(offset))
    for (const status of options.statuses ?? []) params.append("status", status)
    for (const label of options.labels ?? []) {
      if (label.trim()) params.append("label", label.trim())
    }
    const envelope = await api.requestEnvelope<SearchTasksResponse, PageEnvelopeMeta>(
      `/api/v1/search/tasks?${params.toString()}`,
      { signal: options.signal },
    )
    const search = expectRecord<SearchTasksResponse>(envelope.data, "search response data")
    const hits = expectArray<SearchTaskHit>(search.hits, "search hits")
    return {
      tasks: hits.map((hit) => hit.task),
      searchMeta: search.meta,
      page: normalizePageMeta(envelope.meta, { limit, offset }),
    } satisfies SearchTasksResult
  }


export async function searchTasksByStatus(api: ApiTransport, options: SearchTaskOptions & { statuses: TaskStatus[] }) {
    const params = searchTaskParams(api, options)
    const envelope = await api.requestEnvelope<SearchTaskStatusWindowsResponse, PageEnvelopeMeta>(
      `/api/v1/search/tasks/by-status?${params.toString()}`,
      { signal: options.signal },
    )
    const data = expectRecord<SearchTaskStatusWindowsResponse>(envelope.data, "search status windows response data")
    return {
      statuses: expectArray<SearchTaskStatusWindowResponse>(data.statuses, "search status windows").map((entry) => ({
        status: entry.status,
        tasks: expectArray<Task>(entry.tasks, "search status window tasks"),
        searchMeta: entry.search_meta,
        page: normalizePageMeta(entry.page, { limit: options.limit ?? 100, offset: options.offset ?? 0 }),
      })),
    } satisfies SearchTaskStatusWindowsResult
  }
