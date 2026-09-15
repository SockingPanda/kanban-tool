// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-remove-step-headers";

export const ApiRemoveStepHeadersSchema = {"$id":"urn:kanban-tool:schema:api:remove-step-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.remove-step request headers v1","type":"object"} as const;
export type ApiRemoveStepHeadersContract = FromSchema<typeof ApiRemoveStepHeadersSchema>;

export const apiRemoveStepHeadersValidator: ReturnType<typeof createContractValidator<ApiRemoveStepHeadersContract>> = createContractValidator<ApiRemoveStepHeadersContract>(
  "api.remove-step.headers",
  staticValidator,
);

export function parseApiRemoveStepHeaders(value: unknown): ApiRemoveStepHeadersContract {
  if (!apiRemoveStepHeadersValidator(value)) throw new ContractValidationError("api.remove-step.headers", apiRemoveStepHeadersValidator.errors);
  return value;
}
