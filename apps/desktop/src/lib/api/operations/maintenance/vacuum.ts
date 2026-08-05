import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseVacuumReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function vacuum(api: ApiTransport, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/vacuum", {
    method: "POST",
    body: {},
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "vacuum response")
  return parseVacuumReport(envelope.data)
}
