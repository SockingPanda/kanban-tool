// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceImportV30HeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-import-v30-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-import-v30 request headers v1","type":"object"} as const;
export type ApiMaintenanceImportV30HeadersContract = FromSchema<typeof ApiMaintenanceImportV30HeadersSchema>;

export const apiMaintenanceImportV30HeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceImportV30HeadersContract>> = createContractValidator<ApiMaintenanceImportV30HeadersContract>(
  "api.maintenance-import-v30.headers",
  ApiMaintenanceImportV30HeadersSchema,
);

export function parseApiMaintenanceImportV30Headers(value: unknown): ApiMaintenanceImportV30HeadersContract {
  if (!apiMaintenanceImportV30HeadersValidator(value)) throw new ContractValidationError("api.maintenance-import-v30.headers", apiMaintenanceImportV30HeadersValidator.errors);
  return value;
}
