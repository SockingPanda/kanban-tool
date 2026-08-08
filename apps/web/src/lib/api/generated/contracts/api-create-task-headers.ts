// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:create-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.create-task request headers v1","type":"object"} as const;
export type ApiCreateTaskHeadersContract = FromSchema<typeof ApiCreateTaskHeadersSchema>;

export const apiCreateTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiCreateTaskHeadersContract>> = createContractValidator<ApiCreateTaskHeadersContract>(
  "api.create-task.headers",
  ApiCreateTaskHeadersSchema,
);

export function parseApiCreateTaskHeaders(value: unknown): ApiCreateTaskHeadersContract {
  if (!apiCreateTaskHeadersValidator(value)) throw new ContractValidationError("api.create-task.headers", apiCreateTaskHeadersValidator.errors);
  return value;
}
