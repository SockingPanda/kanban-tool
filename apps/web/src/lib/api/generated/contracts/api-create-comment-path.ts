// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateCommentPathSchema = {"$id":"urn:kanban-tool:schema:api:create-comment-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban create comment path v1","type":"object"} as const;
export type ApiCreateCommentPathContract = FromSchema<typeof ApiCreateCommentPathSchema>;

export const apiCreateCommentPathValidator: ReturnType<typeof createContractValidator<ApiCreateCommentPathContract>> = createContractValidator<ApiCreateCommentPathContract>(
  "api.create-comment.path",
  ApiCreateCommentPathSchema,
);

export function parseApiCreateCommentPath(value: unknown): ApiCreateCommentPathContract {
  if (!apiCreateCommentPathValidator(value)) throw new ContractValidationError("api.create-comment.path", apiCreateCommentPathValidator.errors);
  return value;
}
