// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiErrorResponseSchema = {"$defs":{"ApiErrorCode":{"enum":["not_found","conflict","idempotency_conflict","dependency_cycle","invalid_input","feature_not_available","server_unavailable","execution_plan_required","steps_incomplete","claim_token_mismatch","dependency_blocked","claim_conflict","invalid_transition","internal"],"type":"string"},"ErrorBody":{"additionalProperties":false,"properties":{"code":{"$ref":"#/$defs/ApiErrorCode"},"message":{"type":"string"}},"required":["code","message"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:error-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"error":{"$ref":"#/$defs/ErrorBody"}},"required":["error"],"title":"Kanban API error response v1","type":"object"} as const;
export type ApiErrorResponseContract = FromSchema<typeof ApiErrorResponseSchema>;

export const apiErrorResponseValidator: ReturnType<typeof createContractValidator<ApiErrorResponseContract>> = createContractValidator<ApiErrorResponseContract>(
  "api.error.response",
  ApiErrorResponseSchema,
);

export function parseApiErrorResponse(value: unknown): ApiErrorResponseContract {
  if (!apiErrorResponseValidator(value)) throw new ContractValidationError("api.error.response", apiErrorResponseValidator.errors);
  return value;
}
