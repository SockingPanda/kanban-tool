// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiMarkExecutionPlanNotRequiredHeadersSchema = {"$id":"urn:kanban-tool:schema:api:mark-execution-plan-not-required-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.mark-execution-plan-not-required request headers v1","type":"object"} as const;
export type ApiMarkExecutionPlanNotRequiredHeadersContract = FromSchema<typeof ApiMarkExecutionPlanNotRequiredHeadersSchema>;

export const apiMarkExecutionPlanNotRequiredHeadersValidator: ReturnType<typeof createContractValidator<ApiMarkExecutionPlanNotRequiredHeadersContract>> = createContractValidator<ApiMarkExecutionPlanNotRequiredHeadersContract>(
  "api.mark-execution-plan-not-required.headers",
  ApiMarkExecutionPlanNotRequiredHeadersSchema,
);

export function parseApiMarkExecutionPlanNotRequiredHeaders(value: unknown): ApiMarkExecutionPlanNotRequiredHeadersContract {
  if (!apiMarkExecutionPlanNotRequiredHeadersValidator(value)) throw new ContractValidationError("api.mark-execution-plan-not-required.headers", apiMarkExecutionPlanNotRequiredHeadersValidator.errors);
  return value;
}
