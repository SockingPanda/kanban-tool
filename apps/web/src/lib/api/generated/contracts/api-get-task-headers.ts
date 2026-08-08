// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:get-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.get-task request headers v1","type":"object"} as const;
export type ApiGetTaskHeadersContract = FromSchema<typeof ApiGetTaskHeadersSchema>;

export const apiGetTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiGetTaskHeadersContract>> = createContractValidator<ApiGetTaskHeadersContract>(
  "api.get-task.headers",
  ApiGetTaskHeadersSchema,
);

export function parseApiGetTaskHeaders(value: unknown): ApiGetTaskHeadersContract {
  if (!apiGetTaskHeadersValidator(value)) throw new ContractValidationError("api.get-task.headers", apiGetTaskHeadersValidator.errors);
  return value;
}
