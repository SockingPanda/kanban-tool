import type { WebRuntimeConfig } from "../../lib/runtime";

import { parseApiCheckpointResponse } from "../../lib/api/generated/contracts/api-checkpoint-response";

import { parseApiDoctorResponse } from "../../lib/api/generated/contracts/api-doctor-response";

import { parseApiGetStatsQuery } from "../../lib/api/generated/contracts/api-get-stats-query";

import { parseApiGetStatsResponse } from "../../lib/api/generated/contracts/api-get-stats-response";

import { parseApiMaintenanceBackupRequest } from "../../lib/api/generated/contracts/api-maintenance-backup-request";

import { parseApiMaintenanceBackupResponse } from "../../lib/api/generated/contracts/api-maintenance-backup-response";

import { parseApiMaintenanceCleanupResponse } from "../../lib/api/generated/contracts/api-maintenance-cleanup-response";

import { parseApiMaintenanceCleanupRequest } from "../../lib/api/generated/contracts/api-maintenance-cleanup-request";

import { parseApiMaintenanceExportRequest } from "../../lib/api/generated/contracts/api-maintenance-export-request";

import { parseApiMaintenanceExportResponse } from "../../lib/api/generated/contracts/api-maintenance-export-response";

import { parseApiMaintenanceImportRequest } from "../../lib/api/generated/contracts/api-maintenance-import-request";

import { parseApiMaintenanceImportResponse } from "../../lib/api/generated/contracts/api-maintenance-import-response";

import { parseApiMaintenanceRebuildResponse } from "../../lib/api/generated/contracts/api-maintenance-rebuild-response";

import { parseApiMaintenanceRebuildRequest } from "../../lib/api/generated/contracts/api-maintenance-rebuild-request";

import { parseApiMaintenanceRunRequest } from "../../lib/api/generated/contracts/api-maintenance-run-request";

import { parseApiMaintenanceRunResponse } from "../../lib/api/generated/contracts/api-maintenance-run-response";

import { parseApiMaintenanceStatusResponse } from "../../lib/api/generated/contracts/api-maintenance-status-response";

import { parseApiMaintenanceVacuumResponse } from "../../lib/api/generated/contracts/api-maintenance-vacuum-response";

import { parseApiSearchStatusQuery } from "../../lib/api/generated/contracts/api-search-status-query";

import { parseApiSearchStatusResponse } from "../../lib/api/generated/contracts/api-search-status-response";

import { createHttpTransport } from "./http-transport";

import { type MaintenanceApiDependencies, type MaintenanceApi, MaintenanceApiError, type MaintenanceApiTransport, parseResponse, wrapTransportError, encodedBoard, normalizedOwner } from "../../application/data/maintenance-api";

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
    maintenanceRun: (owner, signal) => post(
      "/api/v1/maintenance/run",
      parseApiMaintenanceRunRequest({ owner: normalizedOwner(owner), action: "run" }),
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
