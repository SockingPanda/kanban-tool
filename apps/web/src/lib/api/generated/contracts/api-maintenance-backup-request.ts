// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-maintenance-backup-request";

export const ApiMaintenanceBackupRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-backup-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"path":{"type":"string"}},"required":["path"],"title":"Kanban maintenance backup request v1","type":"object"} as const;
export type ApiMaintenanceBackupRequestContract = FromSchema<typeof ApiMaintenanceBackupRequestSchema>;

export const apiMaintenanceBackupRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceBackupRequestContract>> = createContractValidator<ApiMaintenanceBackupRequestContract>(
  "api.maintenance-backup.request",
  staticValidator,
);

export function parseApiMaintenanceBackupRequest(value: unknown): ApiMaintenanceBackupRequestContract {
  if (!apiMaintenanceBackupRequestValidator(value)) throw new ContractValidationError("api.maintenance-backup.request", apiMaintenanceBackupRequestValidator.errors);
  return value;
}
