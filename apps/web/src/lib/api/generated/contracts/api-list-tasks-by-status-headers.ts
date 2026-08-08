// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTasksByStatusHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-tasks-by-status-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-tasks-by-status request headers v1","type":"object"} as const;
export type ApiListTasksByStatusHeadersContract = FromSchema<typeof ApiListTasksByStatusHeadersSchema>;

export const apiListTasksByStatusHeadersValidator: ReturnType<typeof createContractValidator<ApiListTasksByStatusHeadersContract>> = createContractValidator<ApiListTasksByStatusHeadersContract>(
  "api.list-tasks-by-status.headers",
  ApiListTasksByStatusHeadersSchema,
);

export function parseApiListTasksByStatusHeaders(value: unknown): ApiListTasksByStatusHeadersContract {
  if (!apiListTasksByStatusHeadersValidator(value)) throw new ContractValidationError("api.list-tasks-by-status.headers", apiListTasksByStatusHeadersValidator.errors);
  return value;
}
