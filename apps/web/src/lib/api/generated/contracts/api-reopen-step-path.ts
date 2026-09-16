// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-reopen-step-path";

export const ApiReopenStepPathSchema = {"$id":"urn:kanban-tool:schema:api:reopen-step-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"step_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","step_id"],"title":"Kanban reopen step path v1","type":"object"} as const;
export type ApiReopenStepPathContract = ContractValue<typeof ApiReopenStepPathSchema>;

export const apiReopenStepPathValidator: ReturnType<typeof createContractValidator<ApiReopenStepPathContract>> = createContractValidator<ApiReopenStepPathContract>(
  "api.reopen-step.path",
  staticValidator,
);

export function parseApiReopenStepPath(value: unknown): ApiReopenStepPathContract {
  if (!apiReopenStepPathValidator(value)) throw new ContractValidationError("api.reopen-step.path", apiReopenStepPathValidator.errors);
  return value;
}
