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

import { createRpcTransport } from "./rpc-transport";

import { type MaintenanceApi,type MaintenanceApiDependencies,MaintenanceApiError,normalizedOwner,parseResponse,wrapTransportError } from "../../application/data/maintenance-api";

import type { RpcCall,RpcMethod } from '../../application/data/rpc-transport';
export function createMaintenanceApi(dependencies: MaintenanceApiDependencies = {}, runtime?: WebRuntimeConfig): MaintenanceApi {
  const transport = dependencies.transport ?? (runtime ? createRpcTransport(runtime, dependencies) : null)
  if (!transport) throw new MaintenanceApiError('offline', 'Web maintenance 缺少 runtime transport。')
  async function call<T>(method: RpcMethod, parts: Pick<RpcCall, 'path' | 'query' | 'input'>, parser: (payload: unknown) => { data: T }, contractId: string, signal?: AbortSignal): Promise<T> {
    try { return parseResponse(await transport!.call({ method, ...parts, ...(signal ? { signal } : {}) }), parser, contractId) }
    catch (error) { return wrapTransportError(error) }
  }
  return {
    status: signal => call('MaintenanceStatus', {}, parseApiMaintenanceStatusResponse, 'api.maintenance-status.response', signal),
    doctor: signal => call('Doctor', {}, parseApiDoctorResponse, 'api.doctor.response', signal),
    stats: (board, signal) => call('GetStats', { query: parseApiGetStatsQuery({ board }) }, parseApiGetStatsResponse, 'api.get-stats.response', signal),
    searchStatus: (board, signal) => call('SearchStatus', { query: parseApiSearchStatusQuery({ board }) }, parseApiSearchStatusResponse, 'api.search-status.response', signal),
    checkpoint: signal => call('Checkpoint', {}, parseApiCheckpointResponse, 'api.checkpoint.response', signal),
    backup: (path, signal) => call('MaintenanceBackup', { input: parseApiMaintenanceBackupRequest({ path }) }, parseApiMaintenanceBackupResponse, 'api.maintenance-backup.response', signal),
    exportData: (path, signal) => call('MaintenanceExport', { input: parseApiMaintenanceExportRequest({ path }) }, parseApiMaintenanceExportResponse, 'api.maintenance-export.response', signal),
    importData: (path, replace, signal) => call('MaintenanceImport', { input: parseApiMaintenanceImportRequest({ path, replace }) }, parseApiMaintenanceImportResponse, 'api.maintenance-import.response', signal),
    vacuum: signal => call('MaintenanceVacuum', {}, parseApiMaintenanceVacuumResponse, 'api.maintenance-vacuum.response', signal),
    maintenanceRun: (owner, signal) => call('MaintenanceRun', { input: parseApiMaintenanceRunRequest({ owner: normalizedOwner(owner), action: 'run' }) }, parseApiMaintenanceRunResponse, 'api.maintenance-run.response', signal),
    maintenanceRebuild: (owner, signal) => call('MaintenanceRebuild', { input: parseApiMaintenanceRebuildRequest({ owner: normalizedOwner(owner), action: null }) }, parseApiMaintenanceRebuildResponse, 'api.maintenance-rebuild.response', signal),
    maintenanceCleanup: (owner, signal) => call('MaintenanceCleanup', { input: parseApiMaintenanceCleanupRequest({ owner: normalizedOwner(owner), action: null }) }, parseApiMaintenanceCleanupResponse, 'api.maintenance-cleanup.response', signal),
  }
}
