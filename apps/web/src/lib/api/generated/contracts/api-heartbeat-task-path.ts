// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiHeartbeatTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:heartbeat-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban heartbeat task path v1","type":"object"} as const;
export type ApiHeartbeatTaskPathContract = FromSchema<typeof ApiHeartbeatTaskPathSchema>;

export const apiHeartbeatTaskPathValidator: ReturnType<typeof createContractValidator<ApiHeartbeatTaskPathContract>> = createContractValidator<ApiHeartbeatTaskPathContract>(
  "api.heartbeat-task.path",
  ApiHeartbeatTaskPathSchema,
);

export function parseApiHeartbeatTaskPath(value: unknown): ApiHeartbeatTaskPathContract {
  if (!apiHeartbeatTaskPathValidator(value)) throw new ContractValidationError("api.heartbeat-task.path", apiHeartbeatTaskPathValidator.errors);
  return value;
}
