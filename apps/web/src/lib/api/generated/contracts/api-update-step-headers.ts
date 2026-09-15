// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-update-step-headers";

export const ApiUpdateStepHeadersSchema = {"$id":"urn:kanban-tool:schema:api:update-step-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.update-step request headers v1","type":"object"} as const;
export type ApiUpdateStepHeadersContract = FromSchema<typeof ApiUpdateStepHeadersSchema>;

export const apiUpdateStepHeadersValidator: ReturnType<typeof createContractValidator<ApiUpdateStepHeadersContract>> = createContractValidator<ApiUpdateStepHeadersContract>(
  "api.update-step.headers",
  staticValidator,
);

export function parseApiUpdateStepHeaders(value: unknown): ApiUpdateStepHeadersContract {
  if (!apiUpdateStepHeadersValidator(value)) throw new ContractValidationError("api.update-step.headers", apiUpdateStepHeadersValidator.errors);
  return value;
}
