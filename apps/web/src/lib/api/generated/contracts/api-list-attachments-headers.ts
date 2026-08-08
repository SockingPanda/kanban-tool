// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListAttachmentsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-attachments-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-attachments request headers v1","type":"object"} as const;
export type ApiListAttachmentsHeadersContract = FromSchema<typeof ApiListAttachmentsHeadersSchema>;

export const apiListAttachmentsHeadersValidator: ReturnType<typeof createContractValidator<ApiListAttachmentsHeadersContract>> = createContractValidator<ApiListAttachmentsHeadersContract>(
  "api.list-attachments.headers",
  ApiListAttachmentsHeadersSchema,
);

export function parseApiListAttachmentsHeaders(value: unknown): ApiListAttachmentsHeadersContract {
  if (!apiListAttachmentsHeadersValidator(value)) throw new ContractValidationError("api.list-attachments.headers", apiListAttachmentsHeadersValidator.errors);
  return value;
}
