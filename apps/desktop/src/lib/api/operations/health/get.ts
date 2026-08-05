import { ApiTransport } from "../../transport"
import type { HealthStatus, RequestOptions } from "../../types"

export async function health(api: ApiTransport, options: RequestOptions = {}) {
    return api.request<HealthStatus>("/health", options)
  }
