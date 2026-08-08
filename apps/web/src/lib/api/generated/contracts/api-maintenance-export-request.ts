// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMaintenanceExportRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-export-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"path":{"type":"string"}},"required":["path"],"title":"Kanban maintenance export request v1","type":"object"} as const;
export type ApiMaintenanceExportRequestContract = FromSchema<typeof ApiMaintenanceExportRequestSchema>;

export const apiMaintenanceExportRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceExportRequestContract>> = createContractValidator<ApiMaintenanceExportRequestContract>(
  "api.maintenance-export.request",
  ApiMaintenanceExportRequestSchema,
);

export function parseApiMaintenanceExportRequest(value: unknown): ApiMaintenanceExportRequestContract {
  if (!apiMaintenanceExportRequestValidator(value)) throw new ContractValidationError("api.maintenance-export.request", apiMaintenanceExportRequestValidator.errors);
  return value;
}
