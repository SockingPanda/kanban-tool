// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-promote-task-request";

export const ApiPromoteTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:promote-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]}},"title":"Kanban promote task request v1","type":"object"} as const;
export type ApiPromoteTaskRequestContract = ContractValue<typeof ApiPromoteTaskRequestSchema>;

export const apiPromoteTaskRequestValidator: ReturnType<typeof createContractValidator<ApiPromoteTaskRequestContract>> = createContractValidator<ApiPromoteTaskRequestContract>(
  "api.promote-task.request",
  staticValidator,
);

export function parseApiPromoteTaskRequest(value: unknown): ApiPromoteTaskRequestContract {
  if (!apiPromoteTaskRequestValidator(value)) throw new ContractValidationError("api.promote-task.request", apiPromoteTaskRequestValidator.errors);
  return value;
}
