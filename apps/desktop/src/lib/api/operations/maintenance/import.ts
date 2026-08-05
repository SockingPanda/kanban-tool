import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseImportReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function importData(api: ApiTransport, path: string, replace: boolean, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/import", {
    method: "POST",
    body: { path, replace },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "import response")
  return parseImportReport(envelope.data)
}
