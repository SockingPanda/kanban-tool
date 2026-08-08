// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiGetStatsQuerySchema = {"$id":"urn:kanban-tool:schema:api:get-stats-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"board":{"default":"default","type":"string"}},"title":"Kanban get stats query v1","type":"object"} as const;
export type ApiGetStatsQueryContract = FromSchema<typeof ApiGetStatsQuerySchema>;

export const apiGetStatsQueryValidator: ReturnType<typeof createContractValidator<ApiGetStatsQueryContract>> = createContractValidator<ApiGetStatsQueryContract>(
  "api.get-stats.query",
  ApiGetStatsQuerySchema,
);

export function parseApiGetStatsQuery(value: unknown): ApiGetStatsQueryContract {
  if (!apiGetStatsQueryValidator(value)) throw new ContractValidationError("api.get-stats.query", apiGetStatsQueryValidator.errors);
  return value;
}
