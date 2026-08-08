// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListDependenciesPathSchema = {"$id":"urn:kanban-tool:schema:api:list-dependencies-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban API list dependencies path v1","type":"object"} as const;
export type ApiListDependenciesPathContract = FromSchema<typeof ApiListDependenciesPathSchema>;

export const apiListDependenciesPathValidator: ReturnType<typeof createContractValidator<ApiListDependenciesPathContract>> = createContractValidator<ApiListDependenciesPathContract>(
  "api.list-dependencies.path",
  ApiListDependenciesPathSchema,
);

export function parseApiListDependenciesPath(value: unknown): ApiListDependenciesPathContract {
  if (!apiListDependenciesPathValidator(value)) throw new ContractValidationError("api.list-dependencies.path", apiListDependenciesPathValidator.errors);
  return value;
}
