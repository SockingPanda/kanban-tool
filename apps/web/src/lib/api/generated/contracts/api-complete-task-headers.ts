// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCompleteTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:complete-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.complete-task request headers v1","type":"object"} as const;
export type ApiCompleteTaskHeadersContract = FromSchema<typeof ApiCompleteTaskHeadersSchema>;

export const apiCompleteTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiCompleteTaskHeadersContract>> = createContractValidator<ApiCompleteTaskHeadersContract>(
  "api.complete-task.headers",
  ApiCompleteTaskHeadersSchema,
);

export function parseApiCompleteTaskHeaders(value: unknown): ApiCompleteTaskHeadersContract {
  if (!apiCompleteTaskHeadersValidator(value)) throw new ContractValidationError("api.complete-task.headers", apiCompleteTaskHeadersValidator.errors);
  return value;
}
