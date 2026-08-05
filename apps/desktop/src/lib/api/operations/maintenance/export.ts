import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseExportReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function exportData(api: ApiTransport, path: string, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/export", {
    method: "POST",
    body: { path },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "export response")
  return parseExportReport(envelope.data)
}
