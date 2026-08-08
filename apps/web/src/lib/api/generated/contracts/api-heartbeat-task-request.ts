// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiHeartbeatTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:heartbeat-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"claim_token":{"type":"string"},"note":{"type":["string","null"]},"ttl_ms":{"default":300000,"format":"int64","type":"integer"}},"required":["claim_token"],"title":"Kanban heartbeat task request v1","type":"object"} as const;
export type ApiHeartbeatTaskRequestContract = FromSchema<typeof ApiHeartbeatTaskRequestSchema>;

export const apiHeartbeatTaskRequestValidator: ReturnType<typeof createContractValidator<ApiHeartbeatTaskRequestContract>> = createContractValidator<ApiHeartbeatTaskRequestContract>(
  "api.heartbeat-task.request",
  ApiHeartbeatTaskRequestSchema,
);

export function parseApiHeartbeatTaskRequest(value: unknown): ApiHeartbeatTaskRequestContract {
  if (!apiHeartbeatTaskRequestValidator(value)) throw new ContractValidationError("api.heartbeat-task.request", apiHeartbeatTaskRequestValidator.errors);
  return value;
}
