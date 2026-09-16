// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-get-task-path";

export const ApiGetTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:get-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban get task path v1","type":"object"} as const;
export type ApiGetTaskPathContract = ContractValue<typeof ApiGetTaskPathSchema>;

export const apiGetTaskPathValidator: ReturnType<typeof createContractValidator<ApiGetTaskPathContract>> = createContractValidator<ApiGetTaskPathContract>(
  "api.get-task.path",
  staticValidator,
);

export function parseApiGetTaskPath(value: unknown): ApiGetTaskPathContract {
  if (!apiGetTaskPathValidator(value)) throw new ContractValidationError("api.get-task.path", apiGetTaskPathValidator.errors);
  return value;
}
