// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-promote-task-path";

export const ApiPromoteTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:promote-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban promote task path v1","type":"object"} as const;
export type ApiPromoteTaskPathContract = FromSchema<typeof ApiPromoteTaskPathSchema>;

export const apiPromoteTaskPathValidator: ReturnType<typeof createContractValidator<ApiPromoteTaskPathContract>> = createContractValidator<ApiPromoteTaskPathContract>(
  "api.promote-task.path",
  staticValidator,
);

export function parseApiPromoteTaskPath(value: unknown): ApiPromoteTaskPathContract {
  if (!apiPromoteTaskPathValidator(value)) throw new ContractValidationError("api.promote-task.path", apiPromoteTaskPathValidator.errors);
  return value;
}
