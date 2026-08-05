import { ApiTransport } from "../../transport"
import { expectArray, expectRequiredTotalPageMeta } from "../../parsers"
import { parseTaskStatusEnvelope } from "./parsers"
import { taskListParams } from "./list"
import type { Task, TaskListOptions, TaskStatus, TaskStatusWindowResponse, TaskStatusWindowsResult } from "../../types"

export async function listTasksByStatus(api: ApiTransport, options: TaskListOptions & { statuses: TaskStatus[] }) {
    const params = taskListParams(options)
    const envelope = parseTaskStatusEnvelope(await api.requestRaw(`/api/v1/boards/${api.board}/tasks/by-status?${params.toString()}`, { signal: options.signal }))
    const data = envelope.data
    return {
      statuses: expectArray<TaskStatusWindowResponse>(data.statuses, "task status windows").map((entry) => ({
        status: entry.status,
        tasks: expectArray<Task>(entry.tasks, "task status window tasks"),
        page: expectRequiredTotalPageMeta(entry.page, "task status window page"),
      })),
    } satisfies TaskStatusWindowsResult
  }
