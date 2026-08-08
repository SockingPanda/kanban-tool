// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSpecifyTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:specify-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban specify task path v1","type":"object"} as const;
export type ApiSpecifyTaskPathContract = FromSchema<typeof ApiSpecifyTaskPathSchema>;

export const apiSpecifyTaskPathValidator: ReturnType<typeof createContractValidator<ApiSpecifyTaskPathContract>> = createContractValidator<ApiSpecifyTaskPathContract>(
  "api.specify-task.path",
  ApiSpecifyTaskPathSchema,
);

export function parseApiSpecifyTaskPath(value: unknown): ApiSpecifyTaskPathContract {
  if (!apiSpecifyTaskPathValidator(value)) throw new ContractValidationError("api.specify-task.path", apiSpecifyTaskPathValidator.errors);
  return value;
}
