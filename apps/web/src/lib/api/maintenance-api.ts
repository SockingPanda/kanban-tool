import type { WebRuntimeConfig } from "../runtime"
import {
  parseApiCheckpointResponse,
  type ApiCheckpointResponseContract,
} from "./generated/contracts/api-checkpoint-response"
import {
  parseApiDoctorResponse,
  type ApiDoctorResponseContract,
} from "./generated/contracts/api-doctor-response"
import {
  parseApiGetStatsQuery,
} from "./generated/contracts/api-get-stats-query"
import {
  parseApiGetStatsResponse,
  type ApiGetStatsResponseContract,
} from "./generated/contracts/api-get-stats-response"
import {
  parseApiMaintenanceBackupRequest,
} from "./generated/contracts/api-maintenance-backup-request"
import {
  parseApiMaintenanceBackupResponse,
  type ApiMaintenanceBackupResponseContract,
} from "./generated/contracts/api-maintenance-backup-response"
import {
  parseApiMaintenanceCleanupResponse,
} from "./generated/contracts/api-maintenance-cleanup-response"
import { parseApiMaintenanceCleanupRequest } from "./generated/contracts/api-maintenance-cleanup-request"
import {
  parseApiMaintenanceExportRequest,
} from "./generated/contracts/api-maintenance-export-request"
import {
  parseApiMaintenanceExportResponse,
  type ApiMaintenanceExportResponseContract,
} from "./generated/contracts/api-maintenance-export-response"
import {
  parseApiMaintenanceImportRequest,
} from "./generated/contracts/api-maintenance-import-request"
import {
  parseApiMaintenanceImportResponse,
  type ApiMaintenanceImportResponseContract,
} from "./generated/contracts/api-maintenance-import-response"
import {
  parseApiMaintenanceRebuildResponse,
} from "./generated/contracts/api-maintenance-rebuild-response"
import { parseApiMaintenanceRebuildRequest } from "./generated/contracts/api-maintenance-rebuild-request"
import { parseApiMaintenanceRunRequest } from "./generated/contracts/api-maintenance-run-request"
import {
  parseApiMaintenanceRunResponse,
  type ApiMaintenanceRunResponseContract,
} from "./generated/contracts/api-maintenance-run-response"
import {
  parseApiMaintenanceStatusResponse,
  type ApiMaintenanceStatusResponseContract,
} from "./generated/contracts/api-maintenance-status-response"
import {
  parseApiMaintenanceVacuumResponse,
  type ApiMaintenanceVacuumResponseContract,
} from "./generated/contracts/api-maintenance-vacuum-response"
import {
  parseApiSearchStatusQuery,
} from "./generated/contracts/api-search-status-query"
import {
  parseApiSearchStatusResponse,
  type ApiSearchStatusResponseContract,
} from "./generated/contracts/api-search-status-response"
import { ContractValidationError } from "./generated/runtime"
import {
  createHttpTransport,
  HttpTransportError,
  type HttpTransport,
  type HttpTransportOptions,
  type HttpTransportResponse,
} from "./http-transport"

export type MaintenanceStatus = ApiMaintenanceStatusResponseContract["data"]
export type DoctorReport = ApiDoctorResponseContract["data"]
export type CheckpointReport = ApiCheckpointResponseContract["data"]
export type BackupReport = ApiMaintenanceBackupResponseContract["data"]
export type ExportReport = ApiMaintenanceExportResponseContract["data"]
export type ImportReport = ApiMaintenanceImportResponseContract["data"]
export type VacuumReport = ApiMaintenanceVacuumResponseContract["data"]
export type MaintenanceRunReport = ApiMaintenanceRunResponseContract["data"]
export type SearchStatus = ApiSearchStatusResponseContract["data"]
export type QueueStats = ApiGetStatsResponseContract["data"]

export type MaintenanceApiErrorKind =
  | "offline"
  | "http"
  | "invalid_json"
  | "invalid_contract"
  | "cross_origin"
  | "malformed_url"
  | "invalid_content_type"
  | "response_too_large"

export class MaintenanceApiError extends Error {
  readonly kind: MaintenanceApiErrorKind
  readonly status: number | null
  readonly contractId: string | null

  constructor(
    kind: MaintenanceApiErrorKind,
    message: string,
    options: { status?: number; contractId?: string; cause?: unknown } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "MaintenanceApiError"
    this.kind = kind
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
  }
}

export interface MaintenanceApiTransport {
  readonly get: HttpTransport["get"]
  readonly request?: HttpTransport["request"]
}

export interface MaintenanceApiDependencies extends HttpTransportOptions {
  readonly transport?: MaintenanceApiTransport
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof MaintenanceApiError) throw error
  if (error instanceof HttpTransportError) {
    throw new MaintenanceApiError(error.kind, error.message, {
      status: error.status ?? undefined,
      cause: error,
    })
  }
  throw error
}

function parseResponse<T>(
  response: HttpTransportResponse,
  parser: (payload: unknown) => { data: T },
  contractId: string,
): T {
  try {
    return parser(response.payload).data
  } catch (error) {
    if (error instanceof ContractValidationError) {
      throw new MaintenanceApiError("invalid_contract", "Web maintenance 响应不符合当前协议。", {
        contractId,
        cause: error,
      })
    }
    throw error
  }
}

function encodedBoard(board: string): string {
  return encodeURIComponent(board)
}

function normalizedOwner(owner?: string | null): string | null {
  const value = owner?.trim()
  return value ? value : null
}

export function createMaintenanceApi(
  dependencies: MaintenanceApiDependencies = {},
  runtime?: WebRuntimeConfig,
): MaintenanceApi {
  const selectedTransport = dependencies.transport ?? (runtime ? createHttpTransport(runtime, dependencies) : null)
  if (!selectedTransport) throw new MaintenanceApiError("offline", "Web maintenance 缺少 runtime transport。")
  const transport: MaintenanceApiTransport = selectedTransport

  async function get<T>(
    path: string,
    parser: (payload: unknown) => { data: T },
    contractId: string,
    signal?: AbortSignal,
  ): Promise<T> {
    try {
      return parseResponse(await transport.get(path, signal), parser, contractId)
    } catch (error) {
      return wrapTransportError(error)
    }
  }

  async function post<T>(
    path: string,
    body: unknown,
    parser: (payload: unknown) => { data: T },
    contractId: string,
    signal?: AbortSignal,
  ): Promise<T> {
    if (!transport.request) throw new MaintenanceApiError("offline", "当前 Web transport 不支持 maintenance 写入。")
    try {
      return parseResponse(await transport.request({ method: "POST", path, body, signal }), parser, contractId)
    } catch (error) {
      return wrapTransportError(error)
    }
  }

  return {
    status: (signal) => get("/api/v1/maintenance/status", parseApiMaintenanceStatusResponse, "api.maintenance-status.response", signal),
    doctor: (signal) => get("/api/v1/maintenance/doctor", parseApiDoctorResponse, "api.doctor.response", signal),
    stats: async (board, signal) => {
      const query = parseApiGetStatsQuery({ board })
      return get(`/api/v1/stats?board=${encodedBoard(query.board ?? "default")}`, parseApiGetStatsResponse, "api.get-stats.response", signal)
    },
    searchStatus: async (board, signal) => {
      const query = parseApiSearchStatusQuery({ board })
      return get(`/api/v1/search/status?board=${encodedBoard(query.board ?? "default")}`, parseApiSearchStatusResponse, "api.search-status.response", signal)
    },
    checkpoint: (signal) => post("/api/v1/maintenance/checkpoint", undefined, parseApiCheckpointResponse, "api.checkpoint.response", signal),
    backup: async (path, signal) => {
      const body = parseApiMaintenanceBackupRequest({ path })
      return post("/api/v1/maintenance/backup", body, parseApiMaintenanceBackupResponse, "api.maintenance-backup.response", signal)
    },
    exportData: async (path, signal) => {
      const body = parseApiMaintenanceExportRequest({ path })
      return post("/api/v1/maintenance/export", body, parseApiMaintenanceExportResponse, "api.maintenance-export.response", signal)
    },
    importData: async (path, replace, signal) => {
      const body = parseApiMaintenanceImportRequest({ path, replace })
      return post("/api/v1/maintenance/import", body, parseApiMaintenanceImportResponse, "api.maintenance-import.response", signal)
    },
    vacuum: (signal) => post("/api/v1/maintenance/vacuum", undefined, parseApiMaintenanceVacuumResponse, "api.maintenance-vacuum.response", signal),
    maintenanceRun: (owner, action, signal) => post(
      "/api/v1/maintenance/run",
      parseApiMaintenanceRunRequest({ owner: normalizedOwner(owner), action: normalizedOwner(action) }),
      parseApiMaintenanceRunResponse,
      "api.maintenance-run.response",
      signal,
    ),
    maintenanceRebuild: (owner, signal) => post(
      "/api/v1/maintenance/rebuild",
      parseApiMaintenanceRebuildRequest({ owner: normalizedOwner(owner), action: null }),
      parseApiMaintenanceRebuildResponse,
      "api.maintenance-rebuild.response",
      signal,
    ),
    maintenanceCleanup: (owner, signal) => post(
      "/api/v1/maintenance/cleanup",
      parseApiMaintenanceCleanupRequest({ owner: normalizedOwner(owner), action: null }),
      parseApiMaintenanceCleanupResponse,
      "api.maintenance-cleanup.response",
      signal,
    ),
  }
}

export interface MaintenanceApi {
  status(signal?: AbortSignal): Promise<MaintenanceStatus>
  doctor(signal?: AbortSignal): Promise<DoctorReport>
  stats(board: string, signal?: AbortSignal): Promise<QueueStats>
  searchStatus(board: string, signal?: AbortSignal): Promise<SearchStatus>
  checkpoint(signal?: AbortSignal): Promise<CheckpointReport>
  backup(path: string, signal?: AbortSignal): Promise<BackupReport>
  exportData(path: string, signal?: AbortSignal): Promise<ExportReport>
  importData(path: string, replace: boolean, signal?: AbortSignal): Promise<ImportReport>
  vacuum(signal?: AbortSignal): Promise<VacuumReport>
  maintenanceRun(owner?: string | null, action?: string | null, signal?: AbortSignal): Promise<MaintenanceRunReport>
  maintenanceRebuild(owner?: string | null, signal?: AbortSignal): Promise<MaintenanceRunReport>
  maintenanceCleanup(owner?: string | null, signal?: AbortSignal): Promise<MaintenanceRunReport>
}
