// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-skip-step-headers";

export const ApiSkipStepHeadersSchema = {"$id":"urn:kanban-tool:schema:api:skip-step-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.skip-step request headers v1","type":"object"} as const;
export type ApiSkipStepHeadersContract = ContractValue<typeof ApiSkipStepHeadersSchema>;

export const apiSkipStepHeadersValidator: ReturnType<typeof createContractValidator<ApiSkipStepHeadersContract>> = createContractValidator<ApiSkipStepHeadersContract>(
  "api.skip-step.headers",
  staticValidator,
);

export function parseApiSkipStepHeaders(value: unknown): ApiSkipStepHeadersContract {
  if (!apiSkipStepHeadersValidator(value)) throw new ContractValidationError("api.skip-step.headers", apiSkipStepHeadersValidator.errors);
  return value;
}
