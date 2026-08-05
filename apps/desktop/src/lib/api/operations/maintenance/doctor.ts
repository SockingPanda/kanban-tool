import { ApiTransport } from "../../transport"
import { expectExactKeys } from "../../parsers"
import { parseDoctorReport } from "./parsers"
import type { RequestOptions } from "../../types"

export async function doctor(api: ApiTransport, options: RequestOptions = {}) {
    const envelope = await api.requestEnvelope<unknown>("/api/v1/maintenance/doctor", {
      method: "POST",
      signal: options.signal,
    })
    expectExactKeys(envelope as unknown as Record<string, unknown>, ["data"], "doctor response")
    return parseDoctorReport(envelope.data)
  }
