// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDownloadAttachmentPathSchema = {"$id":"urn:kanban-tool:schema:api:download-attachment-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"attachment_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","attachment_id"],"title":"Kanban API download attachment path v1","type":"object"} as const;
export type ApiDownloadAttachmentPathContract = FromSchema<typeof ApiDownloadAttachmentPathSchema>;

export const apiDownloadAttachmentPathValidator: ReturnType<typeof createContractValidator<ApiDownloadAttachmentPathContract>> = createContractValidator<ApiDownloadAttachmentPathContract>(
  "api.download-attachment.path",
  ApiDownloadAttachmentPathSchema,
);

export function parseApiDownloadAttachmentPath(value: unknown): ApiDownloadAttachmentPathContract {
  if (!apiDownloadAttachmentPathValidator(value)) throw new ContractValidationError("api.download-attachment.path", apiDownloadAttachmentPathValidator.errors);
  return value;
}
