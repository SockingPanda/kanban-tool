import { ApiTransport } from "./transport"
import { parseAddTaskLabelEnvelope, parseLabelSuggestionEnvelope, parseRemoveTaskLabelEnvelope } from "./legacy/parsers"
import type { RequestOptions } from "./types"

export async function addTaskLabel(api: ApiTransport, taskId: string, name: string, options: RequestOptions = {}) {
    return parseAddTaskLabelEnvelope(await api.requestRaw(`/api/v1/tasks/${taskId}/labels`, {
      method: "POST", body: { name, actor: api.actor }, signal: options.signal,
    })).data
  }


export async function suggestTaskLabels(api: ApiTransport,
    taskId: string,
    options: RequestOptions & {
      limit?: number
      candidateLimit?: number
      atomLimit?: number
      maxSelectedLabels?: number
      minScore?: number
    } = {},
  ) {
    const params = new URLSearchParams({ limit: String(options.limit ?? 5) })
    if (typeof options.candidateLimit === "number") {
      params.set("candidate_limit", String(options.candidateLimit))
    }
    if (typeof options.atomLimit === "number") {
      params.set("atom_limit", String(options.atomLimit))
    }
    if (typeof options.maxSelectedLabels === "number") {
      params.set("max_selected_labels", String(options.maxSelectedLabels))
    }
    if (typeof options.minScore === "number") {
      params.set("min_score", String(options.minScore))
    }
    return parseLabelSuggestionEnvelope(await api.requestRaw(
      `/api/v1/tasks/${taskId}/labels/suggestions?${params.toString()}`,
      { signal: options.signal },
    )).data
  }


export async function removeTaskLabel(api: ApiTransport, taskId: string, labelId: string, options: RequestOptions = {}) {
    return parseRemoveTaskLabelEnvelope(await api.requestRaw(`/api/v1/tasks/${taskId}/labels/${labelId}`, {
      method: "DELETE", signal: options.signal,
    })).data
  }
