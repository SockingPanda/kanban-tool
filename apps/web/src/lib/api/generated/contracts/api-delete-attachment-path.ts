// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDeleteAttachmentPathSchema = {"$id":"urn:kanban-tool:schema:api:delete-attachment-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"attachment_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","attachment_id"],"title":"Kanban API delete attachment path v1","type":"object"} as const;
export type ApiDeleteAttachmentPathContract = FromSchema<typeof ApiDeleteAttachmentPathSchema>;

export const apiDeleteAttachmentPathValidator: ReturnType<typeof createContractValidator<ApiDeleteAttachmentPathContract>> = createContractValidator<ApiDeleteAttachmentPathContract>(
  "api.delete-attachment.path",
  ApiDeleteAttachmentPathSchema,
);

export function parseApiDeleteAttachmentPath(value: unknown): ApiDeleteAttachmentPathContract {
  if (!apiDeleteAttachmentPathValidator(value)) throw new ContractValidationError("api.delete-attachment.path", apiDeleteAttachmentPathValidator.errors);
  return value;
}
