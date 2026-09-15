// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-complete-step-path";

export const ApiCompleteStepPathSchema = {"$id":"urn:kanban-tool:schema:api:complete-step-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"step_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","step_id"],"title":"Kanban complete step path v1","type":"object"} as const;
export type ApiCompleteStepPathContract = FromSchema<typeof ApiCompleteStepPathSchema>;

export const apiCompleteStepPathValidator: ReturnType<typeof createContractValidator<ApiCompleteStepPathContract>> = createContractValidator<ApiCompleteStepPathContract>(
  "api.complete-step.path",
  staticValidator,
);

export function parseApiCompleteStepPath(value: unknown): ApiCompleteStepPathContract {
  if (!apiCompleteStepPathValidator(value)) throw new ContractValidationError("api.complete-step.path", apiCompleteStepPathValidator.errors);
  return value;
}
