import { type ApiCheckpointResponseContract } from "../../lib/api/generated/contracts/api-checkpoint-response";

import { type ApiDoctorResponseContract } from "../../lib/api/generated/contracts/api-doctor-response";

import { type ApiGetStatsResponseContract } from "../../lib/api/generated/contracts/api-get-stats-response";

import { type ApiMaintenanceBackupResponseContract } from "../../lib/api/generated/contracts/api-maintenance-backup-response";

import { type ApiMaintenanceExportResponseContract } from "../../lib/api/generated/contracts/api-maintenance-export-response";

import { type ApiMaintenanceImportResponseContract } from "../../lib/api/generated/contracts/api-maintenance-import-response";

import { type ApiMaintenanceRunResponseContract } from "../../lib/api/generated/contracts/api-maintenance-run-response";

import { type ApiMaintenanceStatusResponseContract } from "../../lib/api/generated/contracts/api-maintenance-status-response";

import { type ApiMaintenanceVacuumResponseContract } from "../../lib/api/generated/contracts/api-maintenance-vacuum-response";

import { type ApiSearchStatusResponseContract } from "../../lib/api/generated/contracts/api-search-status-response";

import { ContractValidationError } from "../../lib/api/generated/runtime";

import { RpcTransportError,type RpcTransport,type RpcTransportOptions,type RpcTransportResponse } from "./rpc-transport";

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
  | "invalid_headers"
  | "invalid_content_type"
  | "invalid_bytes"
  | "response_too_large"

export class MaintenanceApiError extends Error {
  readonly kind: MaintenanceApiErrorKind
  readonly code: string | null
  readonly status: number | null
  readonly contractId: string | null

  constructor(
    kind: MaintenanceApiErrorKind,
    message: string,
    options: { code?: string; status?: number; contractId?: string; cause?: unknown } = {},
  ) {
    super(message, { cause: options.cause })
    this.name = "MaintenanceApiError"
    this.kind = kind
    this.code = options.code ?? null
    this.status = options.status ?? null
    this.contractId = options.contractId ?? null
  }
}

export type MaintenanceApiTransport = RpcTransport

export interface MaintenanceApiDependencies extends RpcTransportOptions {
  readonly transport?: MaintenanceApiTransport
}

export function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === "AbortError"
}

export function wrapTransportError(error: unknown): never {
  if (isAbortError(error)) throw error
  if (error instanceof MaintenanceApiError) throw error
  if (error instanceof RpcTransportError) {
    throw new MaintenanceApiError(error.kind, error.message, {
      code: error.apiError?.code,
      status: error.status ?? undefined,
      cause: error,
    })
  }
  throw error
}

export function parseResponse<T>(
  response: RpcTransportResponse,
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

export function normalizedOwner(owner?: string | null): string | null {
  const value = owner?.trim()
  return value ? value : null
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
  maintenanceRun(owner?: string | null, signal?: AbortSignal): Promise<MaintenanceRunReport>
  maintenanceRebuild(owner?: string | null, signal?: AbortSignal): Promise<MaintenanceRunReport>
  maintenanceCleanup(owner?: string | null, signal?: AbortSignal): Promise<MaintenanceRunReport>
}
