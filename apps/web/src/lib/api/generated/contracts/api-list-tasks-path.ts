// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTasksPathSchema = {"$id":"urn:kanban-tool:schema:api:list-tasks-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"minLength":1,"type":"string"}},"required":["board"],"title":"Kanban list tasks path v1","type":"object"} as const;
export type ApiListTasksPathContract = FromSchema<typeof ApiListTasksPathSchema>;

export const apiListTasksPathValidator: ReturnType<typeof createContractValidator<ApiListTasksPathContract>> = createContractValidator<ApiListTasksPathContract>(
  "api.list-tasks.path",
  ApiListTasksPathSchema,
);

export function parseApiListTasksPath(value: unknown): ApiListTasksPathContract {
  if (!apiListTasksPathValidator(value)) throw new ContractValidationError("api.list-tasks.path", apiListTasksPathValidator.errors);
  return value;
}
