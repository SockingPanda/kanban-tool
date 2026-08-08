// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiDeleteAttachmentResponseSchema = {"$defs":{"DeleteResult":{"additionalProperties":false,"properties":{"deleted":{"type":"boolean"}},"required":["deleted"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:delete-attachment-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/DeleteResult"}},"required":["data"],"title":"Kanban API delete attachment response v1","type":"object"} as const;
export type ApiDeleteAttachmentResponseContract = FromSchema<typeof ApiDeleteAttachmentResponseSchema>;

export const apiDeleteAttachmentResponseValidator: ReturnType<typeof createContractValidator<ApiDeleteAttachmentResponseContract>> = createContractValidator<ApiDeleteAttachmentResponseContract>(
  "api.delete-attachment.response",
  ApiDeleteAttachmentResponseSchema,
);

export function parseApiDeleteAttachmentResponse(value: unknown): ApiDeleteAttachmentResponseContract {
  if (!apiDeleteAttachmentResponseValidator(value)) throw new ContractValidationError("api.delete-attachment.response", apiDeleteAttachmentResponseValidator.errors);
  return value;
}
