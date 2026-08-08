// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiArchiveTaskPathSchema = {"$id":"urn:kanban-tool:schema:api:archive-task-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban archive task path v1","type":"object"} as const;
export type ApiArchiveTaskPathContract = FromSchema<typeof ApiArchiveTaskPathSchema>;

export const apiArchiveTaskPathValidator: ReturnType<typeof createContractValidator<ApiArchiveTaskPathContract>> = createContractValidator<ApiArchiveTaskPathContract>(
  "api.archive-task.path",
  ApiArchiveTaskPathSchema,
);

export function parseApiArchiveTaskPath(value: unknown): ApiArchiveTaskPathContract {
  if (!apiArchiveTaskPathValidator(value)) throw new ContractValidationError("api.archive-task.path", apiArchiveTaskPathValidator.errors);
  return value;
}
