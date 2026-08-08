// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSuggestTaskLabelsPathSchema = {"$id":"urn:kanban-tool:schema:api:suggest-task-labels-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Suggest Task Labels Path v1","type":"object"} as const;
export type ApiSuggestTaskLabelsPathContract = FromSchema<typeof ApiSuggestTaskLabelsPathSchema>;

export const apiSuggestTaskLabelsPathValidator: ReturnType<typeof createContractValidator<ApiSuggestTaskLabelsPathContract>> = createContractValidator<ApiSuggestTaskLabelsPathContract>(
  "api.suggest-task-labels.path",
  ApiSuggestTaskLabelsPathSchema,
);

export function parseApiSuggestTaskLabelsPath(value: unknown): ApiSuggestTaskLabelsPathContract {
  if (!apiSuggestTaskLabelsPathValidator(value)) throw new ContractValidationError("api.suggest-task-labels.path", apiSuggestTaskLabelsPathValidator.errors);
  return value;
}
