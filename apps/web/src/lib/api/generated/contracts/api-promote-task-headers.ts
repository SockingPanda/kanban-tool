// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiPromoteTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:promote-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.promote-task request headers v1","type":"object"} as const;
export type ApiPromoteTaskHeadersContract = FromSchema<typeof ApiPromoteTaskHeadersSchema>;

export const apiPromoteTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiPromoteTaskHeadersContract>> = createContractValidator<ApiPromoteTaskHeadersContract>(
  "api.promote-task.headers",
  ApiPromoteTaskHeadersSchema,
);

export function parseApiPromoteTaskHeaders(value: unknown): ApiPromoteTaskHeadersContract {
  if (!apiPromoteTaskHeadersValidator(value)) throw new ContractValidationError("api.promote-task.headers", apiPromoteTaskHeadersValidator.errors);
  return value;
}
