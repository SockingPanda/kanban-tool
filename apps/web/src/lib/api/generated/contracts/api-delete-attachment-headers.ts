// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDeleteAttachmentHeadersSchema = {"$id":"urn:kanban-tool:schema:api:delete-attachment-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.delete-attachment request headers v1","type":"object"} as const;
export type ApiDeleteAttachmentHeadersContract = FromSchema<typeof ApiDeleteAttachmentHeadersSchema>;

export const apiDeleteAttachmentHeadersValidator: ReturnType<typeof createContractValidator<ApiDeleteAttachmentHeadersContract>> = createContractValidator<ApiDeleteAttachmentHeadersContract>(
  "api.delete-attachment.headers",
  ApiDeleteAttachmentHeadersSchema,
);

export function parseApiDeleteAttachmentHeaders(value: unknown): ApiDeleteAttachmentHeadersContract {
  if (!apiDeleteAttachmentHeadersValidator(value)) throw new ContractValidationError("api.delete-attachment.headers", apiDeleteAttachmentHeadersValidator.errors);
  return value;
}
