// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiRemoveDependencyPathSchema = {"$id":"urn:kanban-tool:schema:api:remove-dependency-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"child_task_id":{"type":"string"},"parent_task_id":{"type":"string"}},"required":["child_task_id","parent_task_id"],"title":"Kanban API remove dependency path v1","type":"object"} as const;
export type ApiRemoveDependencyPathContract = FromSchema<typeof ApiRemoveDependencyPathSchema>;

export const apiRemoveDependencyPathValidator: ReturnType<typeof createContractValidator<ApiRemoveDependencyPathContract>> = createContractValidator<ApiRemoveDependencyPathContract>(
  "api.remove-dependency.path",
  ApiRemoveDependencyPathSchema,
);

export function parseApiRemoveDependencyPath(value: unknown): ApiRemoveDependencyPathContract {
  if (!apiRemoveDependencyPathValidator(value)) throw new ContractValidationError("api.remove-dependency.path", apiRemoveDependencyPathValidator.errors);
  return value;
}
