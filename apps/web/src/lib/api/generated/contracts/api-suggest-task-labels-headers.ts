// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSuggestTaskLabelsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:suggest-task-labels-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.suggest-task-labels request headers v1","type":"object"} as const;
export type ApiSuggestTaskLabelsHeadersContract = FromSchema<typeof ApiSuggestTaskLabelsHeadersSchema>;

export const apiSuggestTaskLabelsHeadersValidator: ReturnType<typeof createContractValidator<ApiSuggestTaskLabelsHeadersContract>> = createContractValidator<ApiSuggestTaskLabelsHeadersContract>(
  "api.suggest-task-labels.headers",
  ApiSuggestTaskLabelsHeadersSchema,
);

export function parseApiSuggestTaskLabelsHeaders(value: unknown): ApiSuggestTaskLabelsHeadersContract {
  if (!apiSuggestTaskLabelsHeadersValidator(value)) throw new ContractValidationError("api.suggest-task-labels.headers", apiSuggestTaskLabelsHeadersValidator.errors);
  return value;
}
