// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateStepPathSchema = {"$id":"urn:kanban-tool:schema:api:create-step-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban create step path v1","type":"object"} as const;
export type ApiCreateStepPathContract = FromSchema<typeof ApiCreateStepPathSchema>;

export const apiCreateStepPathValidator: ReturnType<typeof createContractValidator<ApiCreateStepPathContract>> = createContractValidator<ApiCreateStepPathContract>(
  "api.create-step.path",
  ApiCreateStepPathSchema,
);

export function parseApiCreateStepPath(value: unknown): ApiCreateStepPathContract {
  if (!apiCreateStepPathValidator(value)) throw new ContractValidationError("api.create-step.path", apiCreateStepPathValidator.errors);
  return value;
}
