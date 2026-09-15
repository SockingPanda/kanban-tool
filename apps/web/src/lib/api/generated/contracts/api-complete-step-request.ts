// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-complete-step-request";

export const ApiCompleteStepRequestSchema = {"$id":"urn:kanban-tool:schema:api:complete-step-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"note":{"type":"string"}},"required":["note"],"title":"Kanban complete step request v1","type":"object"} as const;
export type ApiCompleteStepRequestContract = FromSchema<typeof ApiCompleteStepRequestSchema>;

export const apiCompleteStepRequestValidator: ReturnType<typeof createContractValidator<ApiCompleteStepRequestContract>> = createContractValidator<ApiCompleteStepRequestContract>(
  "api.complete-step.request",
  staticValidator,
);

export function parseApiCompleteStepRequest(value: unknown): ApiCompleteStepRequestContract {
  if (!apiCompleteStepRequestValidator(value)) throw new ContractValidationError("api.complete-step.request", apiCompleteStepRequestValidator.errors);
  return value;
}
