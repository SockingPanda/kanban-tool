// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiRemoveTaskLabelPathSchema = {"$id":"urn:kanban-tool:schema:api:remove-task-label-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"label_id":{"type":"string"},"task_id":{"type":"string"}},"required":["task_id","label_id"],"title":"Kanban remove task label path v1","type":"object"} as const;
export type ApiRemoveTaskLabelPathContract = FromSchema<typeof ApiRemoveTaskLabelPathSchema>;

export const apiRemoveTaskLabelPathValidator: ReturnType<typeof createContractValidator<ApiRemoveTaskLabelPathContract>> = createContractValidator<ApiRemoveTaskLabelPathContract>(
  "api.remove-task-label.path",
  ApiRemoveTaskLabelPathSchema,
);

export function parseApiRemoveTaskLabelPath(value: unknown): ApiRemoveTaskLabelPathContract {
  if (!apiRemoveTaskLabelPathValidator(value)) throw new ContractValidationError("api.remove-task-label.path", apiRemoveTaskLabelPathValidator.errors);
  return value;
}
