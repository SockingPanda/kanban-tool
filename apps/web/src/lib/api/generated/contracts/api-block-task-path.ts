// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiBlockTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:block-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban block task path v1","type":"object"} as const;
export type ApiBlockTaskPathContract = FromSchema<typeof ApiBlockTaskPathSchema>;

export const apiBlockTaskPathValidator: ReturnType<typeof createContractValidator<ApiBlockTaskPathContract>> = createContractValidator<ApiBlockTaskPathContract>(
  "api.block-task.path",
  ApiBlockTaskPathSchema,
);

export function parseApiBlockTaskPath(value: unknown): ApiBlockTaskPathContract {
  if (!apiBlockTaskPathValidator(value)) throw new ContractValidationError("api.block-task.path", apiBlockTaskPathValidator.errors);
  return value;
}
