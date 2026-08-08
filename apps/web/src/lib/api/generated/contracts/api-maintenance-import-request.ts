// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceImportRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-import-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"path":{"type":"string"},"replace":{"default":false,"type":"boolean"}},"required":["path"],"title":"Kanban maintenance import request v1","type":"object"} as const;
export type ApiMaintenanceImportRequestContract = FromSchema<typeof ApiMaintenanceImportRequestSchema>;

export const apiMaintenanceImportRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceImportRequestContract>> = createContractValidator<ApiMaintenanceImportRequestContract>(
  "api.maintenance-import.request",
  ApiMaintenanceImportRequestSchema,
);

export function parseApiMaintenanceImportRequest(value: unknown): ApiMaintenanceImportRequestContract {
  if (!apiMaintenanceImportRequestValidator(value)) throw new ContractValidationError("api.maintenance-import.request", apiMaintenanceImportRequestValidator.errors);
  return value;
}
