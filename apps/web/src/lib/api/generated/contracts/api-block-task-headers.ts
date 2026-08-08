// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiBlockTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:block-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.block-task request headers v1","type":"object"} as const;
export type ApiBlockTaskHeadersContract = FromSchema<typeof ApiBlockTaskHeadersSchema>;

export const apiBlockTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiBlockTaskHeadersContract>> = createContractValidator<ApiBlockTaskHeadersContract>(
  "api.block-task.headers",
  ApiBlockTaskHeadersSchema,
);

export function parseApiBlockTaskHeaders(value: unknown): ApiBlockTaskHeadersContract {
  if (!apiBlockTaskHeadersValidator(value)) throw new ContractValidationError("api.block-task.headers", apiBlockTaskHeadersValidator.errors);
  return value;
}
