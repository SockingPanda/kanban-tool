// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTasksByStatusPathSchema = {"$id":"urn:kanban-tool:schema:api:list-tasks-by-status-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"minLength":1,"type":"string"}},"required":["board"],"title":"Kanban list tasks by status path v1","type":"object"} as const;
export type ApiListTasksByStatusPathContract = FromSchema<typeof ApiListTasksByStatusPathSchema>;

export const apiListTasksByStatusPathValidator: ReturnType<typeof createContractValidator<ApiListTasksByStatusPathContract>> = createContractValidator<ApiListTasksByStatusPathContract>(
  "api.list-tasks-by-status.path",
  ApiListTasksByStatusPathSchema,
);

export function parseApiListTasksByStatusPath(value: unknown): ApiListTasksByStatusPathContract {
  if (!apiListTasksByStatusPathValidator(value)) throw new ContractValidationError("api.list-tasks-by-status.path", apiListTasksByStatusPathValidator.errors);
  return value;
}
