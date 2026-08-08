// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateCommentHeadersSchema = {"$id":"urn:kanban-tool:schema:api:create-comment-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.create-comment request headers v1","type":"object"} as const;
export type ApiCreateCommentHeadersContract = FromSchema<typeof ApiCreateCommentHeadersSchema>;

export const apiCreateCommentHeadersValidator: ReturnType<typeof createContractValidator<ApiCreateCommentHeadersContract>> = createContractValidator<ApiCreateCommentHeadersContract>(
  "api.create-comment.headers",
  ApiCreateCommentHeadersSchema,
);

export function parseApiCreateCommentHeaders(value: unknown): ApiCreateCommentHeadersContract {
  if (!apiCreateCommentHeadersValidator(value)) throw new ContractValidationError("api.create-comment.headers", apiCreateCommentHeadersValidator.errors);
  return value;
}
