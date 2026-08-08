// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiRemoveDependencyHeadersSchema = {"$id":"urn:kanban-tool:schema:api:remove-dependency-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.remove-dependency request headers v1","type":"object"} as const;
export type ApiRemoveDependencyHeadersContract = FromSchema<typeof ApiRemoveDependencyHeadersSchema>;

export const apiRemoveDependencyHeadersValidator: ReturnType<typeof createContractValidator<ApiRemoveDependencyHeadersContract>> = createContractValidator<ApiRemoveDependencyHeadersContract>(
  "api.remove-dependency.headers",
  ApiRemoveDependencyHeadersSchema,
);

export function parseApiRemoveDependencyHeaders(value: unknown): ApiRemoveDependencyHeadersContract {
  if (!apiRemoveDependencyHeadersValidator(value)) throw new ContractValidationError("api.remove-dependency.headers", apiRemoveDependencyHeadersValidator.errors);
  return value;
}
