// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListDependenciesHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-dependencies-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-dependencies request headers v1","type":"object"} as const;
export type ApiListDependenciesHeadersContract = FromSchema<typeof ApiListDependenciesHeadersSchema>;

export const apiListDependenciesHeadersValidator: ReturnType<typeof createContractValidator<ApiListDependenciesHeadersContract>> = createContractValidator<ApiListDependenciesHeadersContract>(
  "api.list-dependencies.headers",
  ApiListDependenciesHeadersSchema,
);

export function parseApiListDependenciesHeaders(value: unknown): ApiListDependenciesHeadersContract {
  if (!apiListDependenciesHeadersValidator(value)) throw new ContractValidationError("api.list-dependencies.headers", apiListDependenciesHeadersValidator.errors);
  return value;
}
