// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceBackupHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-backup-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-backup request headers v1","type":"object"} as const;
export type ApiMaintenanceBackupHeadersContract = FromSchema<typeof ApiMaintenanceBackupHeadersSchema>;

export const apiMaintenanceBackupHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceBackupHeadersContract>> = createContractValidator<ApiMaintenanceBackupHeadersContract>(
  "api.maintenance-backup.headers",
  ApiMaintenanceBackupHeadersSchema,
);

export function parseApiMaintenanceBackupHeaders(value: unknown): ApiMaintenanceBackupHeadersContract {
  if (!apiMaintenanceBackupHeadersValidator(value)) throw new ContractValidationError("api.maintenance-backup.headers", apiMaintenanceBackupHeadersValidator.errors);
  return value;
}
