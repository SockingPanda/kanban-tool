// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListRunsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:list-runs-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.list-runs request headers v1","type":"object"} as const;
export type ApiListRunsHeadersContract = FromSchema<typeof ApiListRunsHeadersSchema>;

export const apiListRunsHeadersValidator: ReturnType<typeof createContractValidator<ApiListRunsHeadersContract>> = createContractValidator<ApiListRunsHeadersContract>(
  "api.list-runs.headers",
  ApiListRunsHeadersSchema,
);

export function parseApiListRunsHeaders(value: unknown): ApiListRunsHeadersContract {
  if (!apiListRunsHeadersValidator(value)) throw new ContractValidationError("api.list-runs.headers", apiListRunsHeadersValidator.errors);
  return value;
}
