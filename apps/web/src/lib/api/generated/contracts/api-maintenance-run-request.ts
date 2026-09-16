// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-maintenance-run-request";

export const ApiMaintenanceRunRequestSchema = {"$id":"urn:kanban-tool:schema:api:maintenance-run-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"action":{"type":["string","null"]},"owner":{"type":["string","null"]}},"title":"Kanban maintenance run request v1","type":"object"} as const;
export type ApiMaintenanceRunRequestContract = ContractValue<typeof ApiMaintenanceRunRequestSchema>;

export const apiMaintenanceRunRequestValidator: ReturnType<typeof createContractValidator<ApiMaintenanceRunRequestContract>> = createContractValidator<ApiMaintenanceRunRequestContract>(
  "api.maintenance-run.request",
  staticValidator,
);

export function parseApiMaintenanceRunRequest(value: unknown): ApiMaintenanceRunRequestContract {
  if (!apiMaintenanceRunRequestValidator(value)) throw new ContractValidationError("api.maintenance-run.request", apiMaintenanceRunRequestValidator.errors);
  return value;
}
