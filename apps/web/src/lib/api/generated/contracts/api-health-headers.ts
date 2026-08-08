// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiHealthHeadersSchema = {"$id":"urn:kanban-tool:schema:api:health-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.health request headers v1","type":"object"} as const;
export type ApiHealthHeadersContract = FromSchema<typeof ApiHealthHeadersSchema>;

export const apiHealthHeadersValidator: ReturnType<typeof createContractValidator<ApiHealthHeadersContract>> = createContractValidator<ApiHealthHeadersContract>(
  "api.health.headers",
  ApiHealthHeadersSchema,
);

export function parseApiHealthHeaders(value: unknown): ApiHealthHeadersContract {
  if (!apiHealthHeadersValidator(value)) throw new ContractValidationError("api.health.headers", apiHealthHeadersValidator.errors);
  return value;
}
