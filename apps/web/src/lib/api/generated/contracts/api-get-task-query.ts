// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetTaskQuerySchema = {"$id":"urn:kanban-tool:schema:api:get-task-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"include":{"default":null,"type":["string","null"]}},"title":"Kanban get task query v1","type":"object"} as const;
export type ApiGetTaskQueryContract = FromSchema<typeof ApiGetTaskQuerySchema>;

export const apiGetTaskQueryValidator: ReturnType<typeof createContractValidator<ApiGetTaskQueryContract>> = createContractValidator<ApiGetTaskQueryContract>(
  "api.get-task.query",
  ApiGetTaskQuerySchema,
);

export function parseApiGetTaskQuery(value: unknown): ApiGetTaskQueryContract {
  if (!apiGetTaskQueryValidator(value)) throw new ContractValidationError("api.get-task.query", apiGetTaskQueryValidator.errors);
  return value;
}
