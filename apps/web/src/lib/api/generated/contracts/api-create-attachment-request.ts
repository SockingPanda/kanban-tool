// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateAttachmentRequestSchema = {"$id":"urn:kanban-tool:schema:api:create-attachment-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"content":{"default":[],"items":{"format":"uint8","maximum":255,"minimum":0,"type":"integer"},"type":"array"},"content_type":{"type":["string","null"]},"filename":{"type":"string"},"id":{"type":["string","null"]},"rel_path":{"type":["string","null"]},"sha256":{"type":["string","null"]}},"required":["filename"],"title":"Kanban API create attachment request v1","type":"object"} as const;
export type ApiCreateAttachmentRequestContract = FromSchema<typeof ApiCreateAttachmentRequestSchema>;

export const apiCreateAttachmentRequestValidator: ReturnType<typeof createContractValidator<ApiCreateAttachmentRequestContract>> = createContractValidator<ApiCreateAttachmentRequestContract>(
  "api.create-attachment.request",
  ApiCreateAttachmentRequestSchema,
);

export function parseApiCreateAttachmentRequest(value: unknown): ApiCreateAttachmentRequestContract {
  if (!apiCreateAttachmentRequestValidator(value)) throw new ContractValidationError("api.create-attachment.request", apiCreateAttachmentRequestValidator.errors);
  return value;
}
