// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiAddTaskLabelPathSchema = {"$id":"urn:kanban-tool:schema:api:add-task-label-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban add task label path v1","type":"object"} as const;
export type ApiAddTaskLabelPathContract = FromSchema<typeof ApiAddTaskLabelPathSchema>;

export const apiAddTaskLabelPathValidator: ReturnType<typeof createContractValidator<ApiAddTaskLabelPathContract>> = createContractValidator<ApiAddTaskLabelPathContract>(
  "api.add-task-label.path",
  ApiAddTaskLabelPathSchema,
);

export function parseApiAddTaskLabelPath(value: unknown): ApiAddTaskLabelPathContract {
  if (!apiAddTaskLabelPathValidator(value)) throw new ContractValidationError("api.add-task-label.path", apiAddTaskLabelPathValidator.errors);
  return value;
}
