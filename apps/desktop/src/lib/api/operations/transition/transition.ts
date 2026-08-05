import { ApiTransport } from "../../transport"
import { parseClaimEnvelope, parseTransitionTaskEnvelope } from "./parsers"
import type { ClaimResponse, RequestOptions, Task } from "../../types"
export async function transition(api: ApiTransport, task: Task, action: "specify" | "promote" | "claim" | "heartbeat" | "release" | "complete" | "reopen" | "submit-review" | "block" | "unblock" | "archive", body: Record<string, unknown> = {}, options: RequestOptions = {}): Promise<Task | ClaimResponse> {
    const payload = { actor: api.actor, ...body }
    const path = `/api/v1/tasks/${task.id}/transitions/${action}`
    if (action === "specify" || action === "promote" || action === "heartbeat" || action === "release" || action === "submit-review" || action === "complete" || action === "block" || action === "reopen" || action === "unblock" || action === "archive") {
      return parseTransitionTaskEnvelope(await api.requestRaw(path, {
        method: "POST",
        body: payload,
        signal: options.signal,
      }))
    }
    if (action === "claim") {
      return parseClaimEnvelope(await api.requestRaw(path, {
        method: "POST",
        body: payload,
        signal: options.signal,
      }))
    }
    return api.request<Task | ClaimResponse>(path, {
      method: "POST",
      body: payload,
      signal: options.signal,
    })
  }
