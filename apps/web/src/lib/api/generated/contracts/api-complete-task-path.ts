// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCompleteTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:complete-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban complete task path v1","type":"object"} as const;
export type ApiCompleteTaskPathContract = FromSchema<typeof ApiCompleteTaskPathSchema>;

export const apiCompleteTaskPathValidator: ReturnType<typeof createContractValidator<ApiCompleteTaskPathContract>> = createContractValidator<ApiCompleteTaskPathContract>(
  "api.complete-task.path",
  ApiCompleteTaskPathSchema,
);

export function parseApiCompleteTaskPath(value: unknown): ApiCompleteTaskPathContract {
  if (!apiCompleteTaskPathValidator(value)) throw new ContractValidationError("api.complete-task.path", apiCompleteTaskPathValidator.errors);
  return value;
}
