import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseMaintenanceRunReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function maintenanceRebuild(api: ApiTransport, owner: string | null = null, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/rebuild", {
    method: "POST",
    body: { owner, action: null },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "maintenance rebuild response")
  return parseMaintenanceRunReport(envelope.data)
}
