// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiUpdateTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:update-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban update task path v1","type":"object"} as const;
export type ApiUpdateTaskPathContract = FromSchema<typeof ApiUpdateTaskPathSchema>;

export const apiUpdateTaskPathValidator: ReturnType<typeof createContractValidator<ApiUpdateTaskPathContract>> = createContractValidator<ApiUpdateTaskPathContract>(
  "api.update-task.path",
  ApiUpdateTaskPathSchema,
);

export function parseApiUpdateTaskPath(value: unknown): ApiUpdateTaskPathContract {
  if (!apiUpdateTaskPathValidator(value)) throw new ContractValidationError("api.update-task.path", apiUpdateTaskPathValidator.errors);
  return value;
}
