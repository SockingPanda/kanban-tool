// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiAddDependencyHeadersSchema = {"$id":"urn:kanban-tool:schema:api:add-dependency-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.add-dependency request headers v1","type":"object"} as const;
export type ApiAddDependencyHeadersContract = FromSchema<typeof ApiAddDependencyHeadersSchema>;

export const apiAddDependencyHeadersValidator: ReturnType<typeof createContractValidator<ApiAddDependencyHeadersContract>> = createContractValidator<ApiAddDependencyHeadersContract>(
  "api.add-dependency.headers",
  ApiAddDependencyHeadersSchema,
);

export function parseApiAddDependencyHeaders(value: unknown): ApiAddDependencyHeadersContract {
  if (!apiAddDependencyHeadersValidator(value)) throw new ContractValidationError("api.add-dependency.headers", apiAddDependencyHeadersValidator.errors);
  return value;
}
