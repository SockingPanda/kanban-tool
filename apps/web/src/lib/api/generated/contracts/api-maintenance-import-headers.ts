// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-maintenance-import-headers";

export const ApiMaintenanceImportHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-import-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-import request headers v1","type":"object"} as const;
export type ApiMaintenanceImportHeadersContract = FromSchema<typeof ApiMaintenanceImportHeadersSchema>;

export const apiMaintenanceImportHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceImportHeadersContract>> = createContractValidator<ApiMaintenanceImportHeadersContract>(
  "api.maintenance-import.headers",
  staticValidator,
);

export function parseApiMaintenanceImportHeaders(value: unknown): ApiMaintenanceImportHeadersContract {
  if (!apiMaintenanceImportHeadersValidator(value)) throw new ContractValidationError("api.maintenance-import.headers", apiMaintenanceImportHeadersValidator.errors);
  return value;
}
