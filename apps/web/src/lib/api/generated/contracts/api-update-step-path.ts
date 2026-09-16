// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-update-step-path";

export const ApiUpdateStepPathSchema = {"$id":"urn:kanban-tool:schema:api:update-step-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"step_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","step_id"],"title":"Kanban update step path v1","type":"object"} as const;
export type ApiUpdateStepPathContract = ContractValue<typeof ApiUpdateStepPathSchema>;

export const apiUpdateStepPathValidator: ReturnType<typeof createContractValidator<ApiUpdateStepPathContract>> = createContractValidator<ApiUpdateStepPathContract>(
  "api.update-step.path",
  staticValidator,
);

export function parseApiUpdateStepPath(value: unknown): ApiUpdateStepPathContract {
  if (!apiUpdateStepPathValidator(value)) throw new ContractValidationError("api.update-step.path", apiUpdateStepPathValidator.errors);
  return value;
}
