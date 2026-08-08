// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiUpdateTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:update-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.update-task request headers v1","type":"object"} as const;
export type ApiUpdateTaskHeadersContract = FromSchema<typeof ApiUpdateTaskHeadersSchema>;

export const apiUpdateTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiUpdateTaskHeadersContract>> = createContractValidator<ApiUpdateTaskHeadersContract>(
  "api.update-task.headers",
  ApiUpdateTaskHeadersSchema,
);

export function parseApiUpdateTaskHeaders(value: unknown): ApiUpdateTaskHeadersContract {
  if (!apiUpdateTaskHeadersValidator(value)) throw new ContractValidationError("api.update-task.headers", apiUpdateTaskHeadersValidator.errors);
  return value;
}
