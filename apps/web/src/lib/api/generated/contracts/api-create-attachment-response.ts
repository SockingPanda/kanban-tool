// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateAttachmentResponseSchema = {"$defs":{"ApiAttachment":{"additionalProperties":false,"description":"附件内容通过 download endpoint 的 bytes 返回；这个 DTO 仅代表 canonical metadata。","properties":{"board_id":{"type":"string"},"content_type":{"type":["string","null"]},"created_at":{"format":"int64","type":"integer"},"created_by":{"type":"string"},"filename":{"type":"string"},"id":{"type":"string"},"rel_path":{"type":"string"},"sha256":{"type":["string","null"]},"size_bytes":{"format":"int64","type":"integer"},"task_id":{"type":"string"}},"required":["id","board_id","task_id","filename","rel_path","content_type","size_bytes","sha256","created_by","created_at"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:create-attachment-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/ApiAttachment"}},"required":["data"],"title":"Kanban API create attachment response v1","type":"object"} as const;
export type ApiCreateAttachmentResponseContract = FromSchema<typeof ApiCreateAttachmentResponseSchema>;

export const apiCreateAttachmentResponseValidator: ReturnType<typeof createContractValidator<ApiCreateAttachmentResponseContract>> = createContractValidator<ApiCreateAttachmentResponseContract>(
  "api.create-attachment.response",
  ApiCreateAttachmentResponseSchema,
);

export function parseApiCreateAttachmentResponse(value: unknown): ApiCreateAttachmentResponseContract {
  if (!apiCreateAttachmentResponseValidator(value)) throw new ContractValidationError("api.create-attachment.response", apiCreateAttachmentResponseValidator.errors);
  return value;
}
