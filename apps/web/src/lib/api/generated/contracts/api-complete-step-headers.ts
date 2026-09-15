// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-complete-step-headers";

export const ApiCompleteStepHeadersSchema = {"$id":"urn:kanban-tool:schema:api:complete-step-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.complete-step request headers v1","type":"object"} as const;
export type ApiCompleteStepHeadersContract = FromSchema<typeof ApiCompleteStepHeadersSchema>;

export const apiCompleteStepHeadersValidator: ReturnType<typeof createContractValidator<ApiCompleteStepHeadersContract>> = createContractValidator<ApiCompleteStepHeadersContract>(
  "api.complete-step.headers",
  staticValidator,
);

export function parseApiCompleteStepHeaders(value: unknown): ApiCompleteStepHeadersContract {
  if (!apiCompleteStepHeadersValidator(value)) throw new ContractValidationError("api.complete-step.headers", apiCompleteStepHeadersValidator.errors);
  return value;
}
