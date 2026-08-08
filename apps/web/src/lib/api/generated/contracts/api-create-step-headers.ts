// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateStepHeadersSchema = {"$id":"urn:kanban-tool:schema:api:create-step-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.create-step request headers v1","type":"object"} as const;
export type ApiCreateStepHeadersContract = FromSchema<typeof ApiCreateStepHeadersSchema>;

export const apiCreateStepHeadersValidator: ReturnType<typeof createContractValidator<ApiCreateStepHeadersContract>> = createContractValidator<ApiCreateStepHeadersContract>(
  "api.create-step.headers",
  ApiCreateStepHeadersSchema,
);

export function parseApiCreateStepHeaders(value: unknown): ApiCreateStepHeadersContract {
  if (!apiCreateStepHeadersValidator(value)) throw new ContractValidationError("api.create-step.headers", apiCreateStepHeadersValidator.errors);
  return value;
}
