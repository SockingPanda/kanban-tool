// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-skip-step-request";

export const ApiSkipStepRequestSchema = {"$id":"urn:kanban-tool:schema:api:skip-step-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"reason":{"type":"string"}},"required":["reason"],"title":"Kanban skip step request v1","type":"object"} as const;
export type ApiSkipStepRequestContract = FromSchema<typeof ApiSkipStepRequestSchema>;

export const apiSkipStepRequestValidator: ReturnType<typeof createContractValidator<ApiSkipStepRequestContract>> = createContractValidator<ApiSkipStepRequestContract>(
  "api.skip-step.request",
  staticValidator,
);

export function parseApiSkipStepRequest(value: unknown): ApiSkipStepRequestContract {
  if (!apiSkipStepRequestValidator(value)) throw new ContractValidationError("api.skip-step.request", apiSkipStepRequestValidator.errors);
  return value;
}
