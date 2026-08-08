// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiAddDependencyRequestSchema = {"$id":"urn:kanban-tool:schema:api:add-dependency-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"parent_task_id":{"type":"string"}},"required":["parent_task_id"],"title":"Kanban add dependency request v1","type":"object"} as const;
export type ApiAddDependencyRequestContract = FromSchema<typeof ApiAddDependencyRequestSchema>;

export const apiAddDependencyRequestValidator: ReturnType<typeof createContractValidator<ApiAddDependencyRequestContract>> = createContractValidator<ApiAddDependencyRequestContract>(
  "api.add-dependency.request",
  ApiAddDependencyRequestSchema,
);

export function parseApiAddDependencyRequest(value: unknown): ApiAddDependencyRequestContract {
  if (!apiAddDependencyRequestValidator(value)) throw new ContractValidationError("api.add-dependency.request", apiAddDependencyRequestValidator.errors);
  return value;
}
