// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-remove-step-path";

export const ApiRemoveStepPathSchema = {"$id":"urn:kanban-tool:schema:api:remove-step-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"step_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","step_id"],"title":"Kanban remove step path v1","type":"object"} as const;
export type ApiRemoveStepPathContract = ContractValue<typeof ApiRemoveStepPathSchema>;

export const apiRemoveStepPathValidator: ReturnType<typeof createContractValidator<ApiRemoveStepPathContract>> = createContractValidator<ApiRemoveStepPathContract>(
  "api.remove-step.path",
  staticValidator,
);

export function parseApiRemoveStepPath(value: unknown): ApiRemoveStepPathContract {
  if (!apiRemoveStepPathValidator(value)) throw new ContractValidationError("api.remove-step.path", apiRemoveStepPathValidator.errors);
  return value;
}
