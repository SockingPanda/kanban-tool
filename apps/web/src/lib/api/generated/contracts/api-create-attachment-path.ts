// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateAttachmentPathSchema = {"$id":"urn:kanban-tool:schema:api:create-attachment-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban API create attachment path v1","type":"object"} as const;
export type ApiCreateAttachmentPathContract = FromSchema<typeof ApiCreateAttachmentPathSchema>;

export const apiCreateAttachmentPathValidator: ReturnType<typeof createContractValidator<ApiCreateAttachmentPathContract>> = createContractValidator<ApiCreateAttachmentPathContract>(
  "api.create-attachment.path",
  ApiCreateAttachmentPathSchema,
);

export function parseApiCreateAttachmentPath(value: unknown): ApiCreateAttachmentPathContract {
  if (!apiCreateAttachmentPathValidator(value)) throw new ContractValidationError("api.create-attachment.path", apiCreateAttachmentPathValidator.errors);
  return value;
}
