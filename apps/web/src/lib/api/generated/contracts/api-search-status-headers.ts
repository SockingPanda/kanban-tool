// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSearchStatusHeadersSchema = {"$id":"urn:kanban-tool:schema:api:search-status-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.search-status request headers v1","type":"object"} as const;
export type ApiSearchStatusHeadersContract = FromSchema<typeof ApiSearchStatusHeadersSchema>;

export const apiSearchStatusHeadersValidator: ReturnType<typeof createContractValidator<ApiSearchStatusHeadersContract>> = createContractValidator<ApiSearchStatusHeadersContract>(
  "api.search-status.headers",
  ApiSearchStatusHeadersSchema,
);

export function parseApiSearchStatusHeaders(value: unknown): ApiSearchStatusHeadersContract {
  if (!apiSearchStatusHeadersValidator(value)) throw new ContractValidationError("api.search-status.headers", apiSearchStatusHeadersValidator.errors);
  return value;
}
