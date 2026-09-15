// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-reopen-step-request";

export const ApiReopenStepRequestSchema = {"$id":"urn:kanban-tool:schema:api:reopen-step-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"reason":{"type":"string"}},"required":["reason"],"title":"Kanban reopen step request v1","type":"object"} as const;
export type ApiReopenStepRequestContract = FromSchema<typeof ApiReopenStepRequestSchema>;

export const apiReopenStepRequestValidator: ReturnType<typeof createContractValidator<ApiReopenStepRequestContract>> = createContractValidator<ApiReopenStepRequestContract>(
  "api.reopen-step.request",
  staticValidator,
);

export function parseApiReopenStepRequest(value: unknown): ApiReopenStepRequestContract {
  if (!apiReopenStepRequestValidator(value)) throw new ContractValidationError("api.reopen-step.request", apiReopenStepRequestValidator.errors);
  return value;
}
