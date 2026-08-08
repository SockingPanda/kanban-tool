// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTaskLabelsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-task-labels-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban task labels path v1","type":"object"} as const;
export type ApiListTaskLabelsPathContract = FromSchema<typeof ApiListTaskLabelsPathSchema>;

export const apiListTaskLabelsPathValidator: ReturnType<typeof createContractValidator<ApiListTaskLabelsPathContract>> = createContractValidator<ApiListTaskLabelsPathContract>(
  "api.list-task-labels.path",
  ApiListTaskLabelsPathSchema,
);

export function parseApiListTaskLabelsPath(value: unknown): ApiListTaskLabelsPathContract {
  if (!apiListTaskLabelsPathValidator(value)) throw new ContractValidationError("api.list-task-labels.path", apiListTaskLabelsPathValidator.errors);
  return value;
}
