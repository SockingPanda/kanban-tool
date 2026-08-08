// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiUnblockTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:unblock-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.unblock-task request headers v1","type":"object"} as const;
export type ApiUnblockTaskHeadersContract = FromSchema<typeof ApiUnblockTaskHeadersSchema>;

export const apiUnblockTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiUnblockTaskHeadersContract>> = createContractValidator<ApiUnblockTaskHeadersContract>(
  "api.unblock-task.headers",
  ApiUnblockTaskHeadersSchema,
);

export function parseApiUnblockTaskHeaders(value: unknown): ApiUnblockTaskHeadersContract {
  if (!apiUnblockTaskHeadersValidator(value)) throw new ContractValidationError("api.unblock-task.headers", apiUnblockTaskHeadersValidator.errors);
  return value;
}
