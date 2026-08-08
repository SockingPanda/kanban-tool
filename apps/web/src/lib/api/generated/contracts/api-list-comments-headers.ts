// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListCommentsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-comments-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-comments request headers v1","type":"object"} as const;
export type ApiListCommentsHeadersContract = FromSchema<typeof ApiListCommentsHeadersSchema>;

export const apiListCommentsHeadersValidator: ReturnType<typeof createContractValidator<ApiListCommentsHeadersContract>> = createContractValidator<ApiListCommentsHeadersContract>(
  "api.list-comments.headers",
  ApiListCommentsHeadersSchema,
);

export function parseApiListCommentsHeaders(value: unknown): ApiListCommentsHeadersContract {
  if (!apiListCommentsHeadersValidator(value)) throw new ContractValidationError("api.list-comments.headers", apiListCommentsHeadersValidator.errors);
  return value;
}
