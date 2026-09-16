// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import { ContractValidationError, createContractValidator, type ContractValue } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-reopen-step-headers";

export const ApiReopenStepHeadersSchema = {"$id":"urn:kanban-tool:schema:api:reopen-step-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.reopen-step request headers v1","type":"object"} as const;
export type ApiReopenStepHeadersContract = ContractValue<typeof ApiReopenStepHeadersSchema>;

export const apiReopenStepHeadersValidator: ReturnType<typeof createContractValidator<ApiReopenStepHeadersContract>> = createContractValidator<ApiReopenStepHeadersContract>(
  "api.reopen-step.headers",
  staticValidator,
);

export function parseApiReopenStepHeaders(value: unknown): ApiReopenStepHeadersContract {
  if (!apiReopenStepHeadersValidator(value)) throw new ContractValidationError("api.reopen-step.headers", apiReopenStepHeadersValidator.errors);
  return value;
}
