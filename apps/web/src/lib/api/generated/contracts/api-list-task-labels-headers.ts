// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTaskLabelsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-task-labels-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-task-labels request headers v1","type":"object"} as const;
export type ApiListTaskLabelsHeadersContract = FromSchema<typeof ApiListTaskLabelsHeadersSchema>;

export const apiListTaskLabelsHeadersValidator: ReturnType<typeof createContractValidator<ApiListTaskLabelsHeadersContract>> = createContractValidator<ApiListTaskLabelsHeadersContract>(
  "api.list-task-labels.headers",
  ApiListTaskLabelsHeadersSchema,
);

export function parseApiListTaskLabelsHeaders(value: unknown): ApiListTaskLabelsHeadersContract {
  if (!apiListTaskLabelsHeadersValidator(value)) throw new ContractValidationError("api.list-task-labels.headers", apiListTaskLabelsHeadersValidator.errors);
  return value;
}
