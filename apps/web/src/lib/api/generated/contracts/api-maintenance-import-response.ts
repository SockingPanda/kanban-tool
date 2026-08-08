// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceImportResponseSchema = {"$defs":{"ImportReport":{"additionalProperties":false,"properties":{"imported_records":{"format":"uint64","minimum":0,"type":"integer"},"in_path":{"type":"string"},"journal_id":{"type":"string"},"phase":{"type":"string"},"publish_preconditions":{"items":{"type":"string"},"type":"array"},"rebuild_jobs_enqueued":{"format":"uint64","minimum":0,"type":"integer"},"restart_required":{"type":"boolean"},"skipped_records":{"format":"uint64","minimum":0,"type":"integer"},"source_fingerprint":{"type":"string"},"staged_database_path":{"type":["string","null"]},"staged_fingerprint":{"type":["string","null"]},"target_fingerprint_before":{"type":["string","null"]}},"required":["in_path","source_fingerprint","imported_records","skipped_records","rebuild_jobs_enqueued","journal_id","phase","restart_required","staged_database_path","target_fingerprint_before","staged_fingerprint","publish_preconditions"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:maintenance-import-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/ImportReport"}},"required":["data"],"title":"Kanban maintenance import response v1","type":"object"} as const;
export type ApiMaintenanceImportResponseContract = FromSchema<typeof ApiMaintenanceImportResponseSchema>;

export const apiMaintenanceImportResponseValidator: ReturnType<typeof createContractValidator<ApiMaintenanceImportResponseContract>> = createContractValidator<ApiMaintenanceImportResponseContract>(
  "api.maintenance-import.response",
  ApiMaintenanceImportResponseSchema,
);

export function parseApiMaintenanceImportResponse(value: unknown): ApiMaintenanceImportResponseContract {
  if (!apiMaintenanceImportResponseValidator(value)) throw new ContractValidationError("api.maintenance-import.response", apiMaintenanceImportResponseValidator.errors);
  return value;
}
