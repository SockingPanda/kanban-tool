// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetStatsHeadersSchema = {"$id":"urn:kanban-tool:schema:api:get-stats-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.get-stats request headers v1","type":"object"} as const;
export type ApiGetStatsHeadersContract = FromSchema<typeof ApiGetStatsHeadersSchema>;

export const apiGetStatsHeadersValidator: ReturnType<typeof createContractValidator<ApiGetStatsHeadersContract>> = createContractValidator<ApiGetStatsHeadersContract>(
  "api.get-stats.headers",
  ApiGetStatsHeadersSchema,
);

export function parseApiGetStatsHeaders(value: unknown): ApiGetStatsHeadersContract {
  if (!apiGetStatsHeadersValidator(value)) throw new ContractValidationError("api.get-stats.headers", apiGetStatsHeadersValidator.errors);
  return value;
}
