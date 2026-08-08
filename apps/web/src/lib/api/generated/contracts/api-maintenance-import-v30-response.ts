// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceImportV30ResponseSchema = {"$defs":{"LegacyImportReport":{"additionalProperties":false,"properties":{"attachment_count":{"format":"uint64","minimum":0,"type":"integer"},"journal_id":{"type":"string"},"phase":{"type":"string"},"resumed":{"type":"boolean"},"schema_fingerprint":{"type":"string"},"source_fingerprint":{"type":"string"},"source_path":{"type":"string"},"table_counts":{"items":{"$ref":"#/$defs/LegacyImportTableCount"},"type":"array"}},"required":["journal_id","phase","source_path","source_fingerprint","schema_fingerprint","resumed","attachment_count","table_counts"],"type":"object"},"LegacyImportTableCount":{"additionalProperties":false,"properties":{"source_rows":{"format":"uint64","minimum":0,"type":"integer"},"table":{"type":"string"},"target_rows":{"format":"uint64","minimum":0,"type":"integer"}},"required":["table","source_rows","target_rows"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:maintenance-import-v30-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/LegacyImportReport"}},"required":["data"],"title":"Kanban legacy SQLite v30 import response v1","type":"object"} as const;
export type ApiMaintenanceImportV30ResponseContract = FromSchema<typeof ApiMaintenanceImportV30ResponseSchema>;

export const apiMaintenanceImportV30ResponseValidator: ReturnType<typeof createContractValidator<ApiMaintenanceImportV30ResponseContract>> = createContractValidator<ApiMaintenanceImportV30ResponseContract>(
  "api.maintenance-import-v30.response",
  ApiMaintenanceImportV30ResponseSchema,
);

export function parseApiMaintenanceImportV30Response(value: unknown): ApiMaintenanceImportV30ResponseContract {
  if (!apiMaintenanceImportV30ResponseValidator(value)) throw new ContractValidationError("api.maintenance-import-v30.response", apiMaintenanceImportV30ResponseValidator.errors);
  return value;
}
