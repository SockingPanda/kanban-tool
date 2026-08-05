import { ApiTransport } from "./transport"
import { parseSignalEnvelope, parseSignalListEnvelope } from "./legacy/parsers"
import type { RequestOptions, SignalListOptions } from "./types"

function signalSearchParams(options: SignalListOptions) {
  const params = new URLSearchParams({
    include_all: String(options.includeAll ?? false),
    limit: String(options.limit ?? 100),
  })
  for (const status of options.statuses ?? []) params.append("status", status)
  for (const kind of options.kinds ?? []) {
    if (kind.trim()) params.append("kind", kind.trim())
  }
  if (options.task?.trim()) params.set("task", options.task.trim())
  return params
}

export async function listSignals(api: ApiTransport, options: SignalListOptions = {}) {
    const params = signalSearchParams(options)
    const signals = parseSignalListEnvelope(await api.requestRaw(
      `/api/v1/boards/${api.board}/signals?${params.toString()}`,
      { signal: options.signal },
    )).data
    return signals
  }


export async function reviewSignals(api: ApiTransport, options: SignalListOptions = {}) {
    const params = signalSearchParams(options)
    const signals = parseSignalListEnvelope(await api.requestRaw(
      `/api/v1/boards/${api.board}/signals/review?${params.toString()}`,
      { signal: options.signal },
    )).data
    return signals
  }


export async function getSignal(api: ApiTransport, signalId: string, options: RequestOptions = {}) {
    return parseSignalEnvelope(await api.requestRaw(`/api/v1/signals/${encodeURIComponent(signalId)}`, options)).data
  }
