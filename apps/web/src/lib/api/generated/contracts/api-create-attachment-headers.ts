// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateAttachmentHeadersSchema = {"$id":"urn:kanban-tool:schema:api:create-attachment-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.create-attachment request headers v1","type":"object"} as const;
export type ApiCreateAttachmentHeadersContract = FromSchema<typeof ApiCreateAttachmentHeadersSchema>;

export const apiCreateAttachmentHeadersValidator: ReturnType<typeof createContractValidator<ApiCreateAttachmentHeadersContract>> = createContractValidator<ApiCreateAttachmentHeadersContract>(
  "api.create-attachment.headers",
  ApiCreateAttachmentHeadersSchema,
);

export function parseApiCreateAttachmentHeaders(value: unknown): ApiCreateAttachmentHeadersContract {
  if (!apiCreateAttachmentHeadersValidator(value)) throw new ContractValidationError("api.create-attachment.headers", apiCreateAttachmentHeadersValidator.errors);
  return value;
}
