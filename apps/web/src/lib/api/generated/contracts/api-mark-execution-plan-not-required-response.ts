// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMarkExecutionPlanNotRequiredResponseSchema = {"$defs":{"ApiExecutionPlan":{"additionalProperties":false,"properties":{"board_id":{"type":"string"},"reason":{"type":["string","null"]},"state":{"$ref":"#/$defs/ApiExecutionPlanState"},"task_id":{"type":"string"},"updated_at":{"format":"int64","type":"integer"},"updated_by":{"type":"string"}},"required":["board_id","task_id","state","reason","updated_by","updated_at"],"type":"object"},"ApiExecutionPlanState":{"enum":["unplanned","planned","not_required"],"type":"string"}},"$id":"urn:kanban-tool:schema:api:mark-execution-plan-not-required-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/ApiExecutionPlan"}},"required":["data"],"title":"Kanban API mark execution plan not required response v1","type":"object"} as const;
export type ApiMarkExecutionPlanNotRequiredResponseContract = FromSchema<typeof ApiMarkExecutionPlanNotRequiredResponseSchema>;

export const apiMarkExecutionPlanNotRequiredResponseValidator: ReturnType<typeof createContractValidator<ApiMarkExecutionPlanNotRequiredResponseContract>> = createContractValidator<ApiMarkExecutionPlanNotRequiredResponseContract>(
  "api.mark-execution-plan-not-required.response",
  ApiMarkExecutionPlanNotRequiredResponseSchema,
);

export function parseApiMarkExecutionPlanNotRequiredResponse(value: unknown): ApiMarkExecutionPlanNotRequiredResponseContract {
  if (!apiMarkExecutionPlanNotRequiredResponseValidator(value)) throw new ContractValidationError("api.mark-execution-plan-not-required.response", apiMarkExecutionPlanNotRequiredResponseValidator.errors);
  return value;
}
