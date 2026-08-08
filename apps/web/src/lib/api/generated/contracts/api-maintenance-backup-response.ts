// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceBackupResponseSchema = {"$defs":{"BackupReport":{"additionalProperties":false,"properties":{"bytes":{"format":"uint64","minimum":0,"type":"integer"},"checksum_sha256":{"type":"string"},"out_path":{"type":"string"},"source_fingerprint":{"type":"string"}},"required":["out_path","checksum_sha256","bytes","source_fingerprint"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:maintenance-backup-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/BackupReport"}},"required":["data"],"title":"Kanban maintenance backup response v1","type":"object"} as const;
export type ApiMaintenanceBackupResponseContract = FromSchema<typeof ApiMaintenanceBackupResponseSchema>;

export const apiMaintenanceBackupResponseValidator: ReturnType<typeof createContractValidator<ApiMaintenanceBackupResponseContract>> = createContractValidator<ApiMaintenanceBackupResponseContract>(
  "api.maintenance-backup.response",
  ApiMaintenanceBackupResponseSchema,
);

export function parseApiMaintenanceBackupResponse(value: unknown): ApiMaintenanceBackupResponseContract {
  if (!apiMaintenanceBackupResponseValidator(value)) throw new ContractValidationError("api.maintenance-backup.response", apiMaintenanceBackupResponseValidator.errors);
  return value;
}
