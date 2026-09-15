// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-skip-step-path";

export const ApiSkipStepPathSchema = {"$id":"urn:kanban-tool:schema:api:skip-step-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"step_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","step_id"],"title":"Kanban skip step path v1","type":"object"} as const;
export type ApiSkipStepPathContract = FromSchema<typeof ApiSkipStepPathSchema>;

export const apiSkipStepPathValidator: ReturnType<typeof createContractValidator<ApiSkipStepPathContract>> = createContractValidator<ApiSkipStepPathContract>(
  "api.skip-step.path",
  staticValidator,
);

export function parseApiSkipStepPath(value: unknown): ApiSkipStepPathContract {
  if (!apiSkipStepPathValidator(value)) throw new ContractValidationError("api.skip-step.path", apiSkipStepPathValidator.errors);
  return value;
}
