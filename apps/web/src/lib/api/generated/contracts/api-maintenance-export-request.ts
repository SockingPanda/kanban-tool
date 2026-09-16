// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-maintenance-export-request";

export const ApiMaintenanceExportRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-export-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"path":{"type":"string"}},"required":["path"],"title":"Kanban maintenance export request v1","type":"object"} as const;
export type ApiMaintenanceExportRequestContract = ContractValue<typeof ApiMaintenanceExportRequestSchema>;

export const apiMaintenanceExportRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceExportRequestContract>> = createContractValidator<ApiMaintenanceExportRequestContract>(
  "api.maintenance-export.request",
  staticValidator,
);

export function parseApiMaintenanceExportRequest(value: unknown): ApiMaintenanceExportRequestContract {
  if (!apiMaintenanceExportRequestValidator(value)) throw new ContractValidationError("api.maintenance-export.request", apiMaintenanceExportRequestValidator.errors);
  return value;
}
