// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDownloadAttachmentHeadersSchema = {"$id":"urn:kanban-tool:schema:api:download-attachment-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.download-attachment request headers v1","type":"object"} as const;
export type ApiDownloadAttachmentHeadersContract = FromSchema<typeof ApiDownloadAttachmentHeadersSchema>;

export const apiDownloadAttachmentHeadersValidator: ReturnType<typeof createContractValidator<ApiDownloadAttachmentHeadersContract>> = createContractValidator<ApiDownloadAttachmentHeadersContract>(
  "api.download-attachment.headers",
  ApiDownloadAttachmentHeadersSchema,
);

export function parseApiDownloadAttachmentHeaders(value: unknown): ApiDownloadAttachmentHeadersContract {
  if (!apiDownloadAttachmentHeadersValidator(value)) throw new ContractValidationError("api.download-attachment.headers", apiDownloadAttachmentHeadersValidator.errors);
  return value;
}
