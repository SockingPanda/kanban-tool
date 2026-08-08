// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateCommentRequestSchema = {"$defs":{"CommentAuthorType":{"enum":["user","agent"],"type":"string"},"CommentKind":{"enum":["note","decision","signal"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:create-comment-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"agent_type":{"type":["string","null"]},"author":{"type":["string","null"]},"author_type":{"anyOf":[{"$ref":"#/$defs/CommentAuthorType"},{"type":"null"}]},"body":{"type":"string"},"idempotency_key":{"description":"作用域限定在此任务上的 entity-local 重试 key。","type":["string","null"]},"kind":{"anyOf":[{"$ref":"#/$defs/CommentKind"},{"type":"null"}]},"metadata":{"additionalProperties":true,"type":["object","null"]}},"required":["body"],"title":"Kanban create comment request v1","type":"object"} as const;
export type ApiCreateCommentRequestContract = FromSchema<typeof ApiCreateCommentRequestSchema>;

export const apiCreateCommentRequestValidator: ReturnType<typeof createContractValidator<ApiCreateCommentRequestContract>> = createContractValidator<ApiCreateCommentRequestContract>(
  "api.create-comment.request",
  ApiCreateCommentRequestSchema,
);

export function parseApiCreateCommentRequest(value: unknown): ApiCreateCommentRequestContract {
  if (!apiCreateCommentRequestValidator(value)) throw new ContractValidationError("api.create-comment.request", apiCreateCommentRequestValidator.errors);
  return value;
}
