import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseMaintenanceStatusReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function maintenanceStatus(api: ApiTransport, options: RequestOptions = {}) {
  const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/status", options)
  expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "maintenance status response")
  return parseMaintenanceStatusReport(envelope.data)
}
