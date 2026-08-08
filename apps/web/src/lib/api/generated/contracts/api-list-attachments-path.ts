// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListAttachmentsPathSchema = {"$id":"urn:kanban-tool:schema:api:list-attachments-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban API list attachments path v1","type":"object"} as const;
export type ApiListAttachmentsPathContract = FromSchema<typeof ApiListAttachmentsPathSchema>;

export const apiListAttachmentsPathValidator: ReturnType<typeof createContractValidator<ApiListAttachmentsPathContract>> = createContractValidator<ApiListAttachmentsPathContract>(
  "api.list-attachments.path",
  ApiListAttachmentsPathSchema,
);

export function parseApiListAttachmentsPath(value: unknown): ApiListAttachmentsPathContract {
  if (!apiListAttachmentsPathValidator(value)) throw new ContractValidationError("api.list-attachments.path", apiListAttachmentsPathValidator.errors);
  return value;
}
