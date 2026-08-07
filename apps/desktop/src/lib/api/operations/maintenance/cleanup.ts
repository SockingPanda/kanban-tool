import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseMaintenanceRunReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function maintenanceCleanup(api: ApiTransport, owner: string | null = null, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/cleanup", {
    method: "POST",
    body: { owner, action: null },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "maintenance cleanup response")
  return parseMaintenanceRunReport(envelope.data)
}
