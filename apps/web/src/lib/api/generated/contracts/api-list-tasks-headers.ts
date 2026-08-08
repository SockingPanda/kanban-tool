// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTasksHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-tasks-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-tasks request headers v1","type":"object"} as const;
export type ApiListTasksHeadersContract = FromSchema<typeof ApiListTasksHeadersSchema>;

export const apiListTasksHeadersValidator: ReturnType<typeof createContractValidator<ApiListTasksHeadersContract>> = createContractValidator<ApiListTasksHeadersContract>(
  "api.list-tasks.headers",
  ApiListTasksHeadersSchema,
);

export function parseApiListTasksHeaders(value: unknown): ApiListTasksHeadersContract {
  if (!apiListTasksHeadersValidator(value)) throw new ContractValidationError("api.list-tasks.headers", apiListTasksHeadersValidator.errors);
  return value;
}
