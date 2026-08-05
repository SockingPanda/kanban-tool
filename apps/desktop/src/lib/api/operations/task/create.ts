import { ApiTransport, newClientTaskId } from "../../transport"
import { expectExactKeys, expectRecord } from "../../parsers"
import { parseApiTask } from "./parsers"
import type { CreateTaskInput, RequestOptions } from "../../types"

export async function createTask(api: ApiTransport, input: CreateTaskInput, options: RequestOptions = {}) {
    const taskId = input.taskId ?? newClientTaskId()
    const envelope = await api.requestEnvelope<unknown>(`/api/v1/boards/${api.board}/tasks`, {
      method: "POST",
      body: {
        task_id: taskId,
        idempotency_key: input.idempotencyKey ?? `task.create:${taskId}`,
        title: input.title,
        description: input.description ?? null,
        status: input.status ?? undefined,
        actor: api.actor,
      },
      signal: options.signal,
    })
    const record = expectRecord<Record<string, unknown>>(envelope, "create task response")
    expectExactKeys(record, ["data"], "create task response")
    return parseApiTask(record.data, "create task response data")
  }
