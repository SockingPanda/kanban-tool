// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateCommentResponseSchema = {"$defs":{"ApiComment":{"additionalProperties":false,"properties":{"agent_type":{"type":["string","null"]},"author":{"type":"string"},"author_type":{"$ref":"#/$defs/CommentAuthorType"},"board_id":{"type":"string"},"body":{"type":"string"},"created_at":{"format":"int64","type":"integer"},"id":{"type":"string"},"kind":{"$ref":"#/$defs/CommentKind"},"metadata":{"additionalProperties":true,"type":"object"},"task_id":{"type":"string"}},"required":["id","board_id","task_id","author","author_type","agent_type","body","kind","metadata","created_at"],"type":"object"},"CommentAuthorType":{"enum":["user","agent"],"type":"string"},"CommentKind":{"enum":["note","decision","signal"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:create-comment-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/ApiComment"}},"required":["data"],"title":"Kanban create comment response v1","type":"object"} as const;
export type ApiCreateCommentResponseContract = FromSchema<typeof ApiCreateCommentResponseSchema>;

export const apiCreateCommentResponseValidator: ReturnType<typeof createContractValidator<ApiCreateCommentResponseContract>> = createContractValidator<ApiCreateCommentResponseContract>(
  "api.create-comment.response",
  ApiCreateCommentResponseSchema,
);

export function parseApiCreateCommentResponse(value: unknown): ApiCreateCommentResponseContract {
  if (!apiCreateCommentResponseValidator(value)) throw new ContractValidationError("api.create-comment.response", apiCreateCommentResponseValidator.errors);
  return value;
}
