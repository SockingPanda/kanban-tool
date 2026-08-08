// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDownloadAttachmentResponseSchema = {"$id":"urn:kanban-tool:schema:api:download-attachment-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","items":{"format":"uint8","maximum":255,"minimum":0,"type":"integer"},"title":"Kanban API download attachment bytes v1","type":"array"} as const;
export type ApiDownloadAttachmentResponseContract = FromSchema<typeof ApiDownloadAttachmentResponseSchema>;

export const apiDownloadAttachmentResponseValidator: ReturnType<typeof createContractValidator<ApiDownloadAttachmentResponseContract>> = createContractValidator<ApiDownloadAttachmentResponseContract>(
  "api.download-attachment.response",
  ApiDownloadAttachmentResponseSchema,
);

export function parseApiDownloadAttachmentResponse(value: unknown): ApiDownloadAttachmentResponseContract {
  if (!apiDownloadAttachmentResponseValidator(value)) throw new ContractValidationError("api.download-attachment.response", apiDownloadAttachmentResponseValidator.errors);
  return value;
}
