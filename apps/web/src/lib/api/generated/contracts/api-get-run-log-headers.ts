// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetRunLogHeadersSchema = {"$id":"urn:kanban-tool:schema:api:get-run-log-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.get-run-log request headers v1","type":"object"} as const;
export type ApiGetRunLogHeadersContract = FromSchema<typeof ApiGetRunLogHeadersSchema>;

export const apiGetRunLogHeadersValidator: ReturnType<typeof createContractValidator<ApiGetRunLogHeadersContract>> = createContractValidator<ApiGetRunLogHeadersContract>(
  "api.get-run-log.headers",
  ApiGetRunLogHeadersSchema,
);

export function parseApiGetRunLogHeaders(value: unknown): ApiGetRunLogHeadersContract {
  if (!apiGetRunLogHeadersValidator(value)) throw new ContractValidationError("api.get-run-log.headers", apiGetRunLogHeadersValidator.errors);
  return value;
}
