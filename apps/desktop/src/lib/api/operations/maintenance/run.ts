import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseMaintenanceRunReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function maintenanceRun(
  api: ApiTransport,
  owner: string | null = null,
  action: string | null = null,
  options: RequestOptions = {},
) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/run", {
    method: "POST",
    body: { owner, action },
    signal: options.signal,
  })
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "maintenance run response")
  return parseMaintenanceRunReport(envelope.data)
}
