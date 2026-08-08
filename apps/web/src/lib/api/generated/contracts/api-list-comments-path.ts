// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListCommentsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-comments-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban list comments path v1","type":"object"} as const;
export type ApiListCommentsPathContract = FromSchema<typeof ApiListCommentsPathSchema>;

export const apiListCommentsPathValidator: ReturnType<typeof createContractValidator<ApiListCommentsPathContract>> = createContractValidator<ApiListCommentsPathContract>(
  "api.list-comments.path",
  ApiListCommentsPathSchema,
);

export function parseApiListCommentsPath(value: unknown): ApiListCommentsPathContract {
  if (!apiListCommentsPathValidator(value)) throw new ContractValidationError("api.list-comments.path", apiListCommentsPathValidator.errors);
  return value;
}
