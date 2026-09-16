// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-update-step-request";

export const ApiUpdateStepRequestSchema = {"$id":"urn:kanban-tool:schema:api:update-step-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"body":{"type":["string","null"]},"linked_task_ref":{"type":["string","null"]},"position":{"format":"int64","type":["integer","null"]},"required":{"type":["boolean","null"]},"title":{"type":["string","null"]},"unlink_task":{"default":false,"type":"boolean"}},"title":"Kanban update step request v1","type":"object"} as const;
export type ApiUpdateStepRequestContract = ContractValue<typeof ApiUpdateStepRequestSchema>;

export const apiUpdateStepRequestValidator: ReturnType<typeof createContractValidator<ApiUpdateStepRequestContract>> = createContractValidator<ApiUpdateStepRequestContract>(
  "api.update-step.request",
  staticValidator,
);

export function parseApiUpdateStepRequest(value: unknown): ApiUpdateStepRequestContract {
  if (!apiUpdateStepRequestValidator(value)) throw new ContractValidationError("api.update-step.request", apiUpdateStepRequestValidator.errors);
  return value;
}
