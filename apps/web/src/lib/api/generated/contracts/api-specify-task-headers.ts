// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSpecifyTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:specify-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.specify-task request headers v1","type":"object"} as const;
export type ApiSpecifyTaskHeadersContract = FromSchema<typeof ApiSpecifyTaskHeadersSchema>;

export const apiSpecifyTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiSpecifyTaskHeadersContract>> = createContractValidator<ApiSpecifyTaskHeadersContract>(
  "api.specify-task.headers",
  ApiSpecifyTaskHeadersSchema,
);

export function parseApiSpecifyTaskHeaders(value: unknown): ApiSpecifyTaskHeadersContract {
  if (!apiSpecifyTaskHeadersValidator(value)) throw new ContractValidationError("api.specify-task.headers", apiSpecifyTaskHeadersValidator.errors);
  return value;
}
