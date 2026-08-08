// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMarkExecutionPlanNotRequiredPathSchema = {"$id":"urn:kanban-tool:schema:api:mark-execution-plan-not-required-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban API mark execution plan not required path v1","type":"object"} as const;
export type ApiMarkExecutionPlanNotRequiredPathContract = FromSchema<typeof ApiMarkExecutionPlanNotRequiredPathSchema>;

export const apiMarkExecutionPlanNotRequiredPathValidator: ReturnType<typeof createContractValidator<ApiMarkExecutionPlanNotRequiredPathContract>> = createContractValidator<ApiMarkExecutionPlanNotRequiredPathContract>(
  "api.mark-execution-plan-not-required.path",
  ApiMarkExecutionPlanNotRequiredPathSchema,
);

export function parseApiMarkExecutionPlanNotRequiredPath(value: unknown): ApiMarkExecutionPlanNotRequiredPathContract {
  if (!apiMarkExecutionPlanNotRequiredPathValidator(value)) throw new ContractValidationError("api.mark-execution-plan-not-required.path", apiMarkExecutionPlanNotRequiredPathValidator.errors);
  return value;
}
