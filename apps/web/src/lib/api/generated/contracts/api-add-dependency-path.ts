// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiAddDependencyPathSchema = {"$id":"urn:kanban-tool:schema:api:add-dependency-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban API add dependency path v1","type":"object"} as const;
export type ApiAddDependencyPathContract = FromSchema<typeof ApiAddDependencyPathSchema>;

export const apiAddDependencyPathValidator: ReturnType<typeof createContractValidator<ApiAddDependencyPathContract>> = createContractValidator<ApiAddDependencyPathContract>(
  "api.add-dependency.path",
  ApiAddDependencyPathSchema,
);

export function parseApiAddDependencyPath(value: unknown): ApiAddDependencyPathContract {
  if (!apiAddDependencyPathValidator(value)) throw new ContractValidationError("api.add-dependency.path", apiAddDependencyPathValidator.errors);
  return value;
}
