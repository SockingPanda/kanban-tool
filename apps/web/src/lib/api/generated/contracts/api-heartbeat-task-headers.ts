// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiHeartbeatTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:heartbeat-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.heartbeat-task request headers v1","type":"object"} as const;
export type ApiHeartbeatTaskHeadersContract = FromSchema<typeof ApiHeartbeatTaskHeadersSchema>;

export const apiHeartbeatTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiHeartbeatTaskHeadersContract>> = createContractValidator<ApiHeartbeatTaskHeadersContract>(
  "api.heartbeat-task.headers",
  ApiHeartbeatTaskHeadersSchema,
);

export function parseApiHeartbeatTaskHeaders(value: unknown): ApiHeartbeatTaskHeadersContract {
  if (!apiHeartbeatTaskHeadersValidator(value)) throw new ContractValidationError("api.heartbeat-task.headers", apiHeartbeatTaskHeadersValidator.errors);
  return value;
}
