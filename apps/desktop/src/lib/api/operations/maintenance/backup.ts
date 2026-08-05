import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseBackupReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function backup(api: ApiTransport, path: string, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/backup", {
    method: "POST",
    body: { path },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "backup response")
  return parseBackupReport(envelope.data)
}
