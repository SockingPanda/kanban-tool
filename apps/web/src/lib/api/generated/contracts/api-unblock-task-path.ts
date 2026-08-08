// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiUnblockTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:unblock-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban unblock task path v1","type":"object"} as const;
export type ApiUnblockTaskPathContract = FromSchema<typeof ApiUnblockTaskPathSchema>;

export const apiUnblockTaskPathValidator: ReturnType<typeof createContractValidator<ApiUnblockTaskPathContract>> = createContractValidator<ApiUnblockTaskPathContract>(
  "api.unblock-task.path",
  ApiUnblockTaskPathSchema,
);

export function parseApiUnblockTaskPath(value: unknown): ApiUnblockTaskPathContract {
  if (!apiUnblockTaskPathValidator(value)) throw new ContractValidationError("api.unblock-task.path", apiUnblockTaskPathValidator.errors);
  return value;
}
