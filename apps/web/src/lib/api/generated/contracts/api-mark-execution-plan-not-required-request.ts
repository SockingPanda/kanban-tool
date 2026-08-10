// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-mark-execution-plan-not-required-request";

export const ApiMarkExecutionPlanNotRequiredRequestSchema = {"$id":"urn:kanban-tool:schema:api:mark-execution-plan-not-required-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"reason":{"type":"string"}},"required":["reason"],"title":"Kanban API mark execution plan not required request v1","type":"object"} as const;
export type ApiMarkExecutionPlanNotRequiredRequestContract = FromSchema<typeof ApiMarkExecutionPlanNotRequiredRequestSchema>;

export const apiMarkExecutionPlanNotRequiredRequestValidator: ReturnType<typeof createContractValidator<ApiMarkExecutionPlanNotRequiredRequestContract>> = createContractValidator<ApiMarkExecutionPlanNotRequiredRequestContract>(
  "api.mark-execution-plan-not-required.request",
  staticValidator,
);

export function parseApiMarkExecutionPlanNotRequiredRequest(value: unknown): ApiMarkExecutionPlanNotRequiredRequestContract {
  if (!apiMarkExecutionPlanNotRequiredRequestValidator(value)) throw new ContractValidationError("api.mark-execution-plan-not-required.request", apiMarkExecutionPlanNotRequiredRequestValidator.errors);
  return value;
}
