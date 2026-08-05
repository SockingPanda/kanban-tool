import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseCheckpointReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function checkpoint(api: ApiTransport, options: RequestOptions = {}) {
    const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/checkpoint", {
      method: "POST",
      signal: options.signal,
    })
    expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "checkpoint response")
    return parseCheckpointReport(envelope.data)
  }
