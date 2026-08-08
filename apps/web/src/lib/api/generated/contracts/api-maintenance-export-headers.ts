// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceExportHeadersSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-export-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"}},"required":["Content-Type"],"title":"Kanban api.maintenance-export request headers v1","type":"object"} as const;
export type ApiMaintenanceExportHeadersContract = FromSchema<typeof ApiMaintenanceExportHeadersSchema>;

export const apiMaintenanceExportHeadersValidator: ReturnType<typeof createContractValidator<ApiMaintenanceExportHeadersContract>> = createContractValidator<ApiMaintenanceExportHeadersContract>(
  "api.maintenance-export.headers",
  ApiMaintenanceExportHeadersSchema,
);

export function parseApiMaintenanceExportHeaders(value: unknown): ApiMaintenanceExportHeadersContract {
  if (!apiMaintenanceExportHeadersValidator(value)) throw new ContractValidationError("api.maintenance-export.headers", apiMaintenanceExportHeadersValidator.errors);
  return value;
}
