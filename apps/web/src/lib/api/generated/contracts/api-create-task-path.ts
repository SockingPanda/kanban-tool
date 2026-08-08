// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:create-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"type":"string"}},"required":["board"],"title":"Kanban create task path v1","type":"object"} as const;
export type ApiCreateTaskPathContract = FromSchema<typeof ApiCreateTaskPathSchema>;

export const apiCreateTaskPathValidator: ReturnType<typeof createContractValidator<ApiCreateTaskPathContract>> = createContractValidator<ApiCreateTaskPathContract>(
  "api.create-task.path",
  ApiCreateTaskPathSchema,
);

export function parseApiCreateTaskPath(value: unknown): ApiCreateTaskPathContract {
  if (!apiCreateTaskPathValidator(value)) throw new ContractValidationError("api.create-task.path", apiCreateTaskPathValidator.errors);
  return value;
}
