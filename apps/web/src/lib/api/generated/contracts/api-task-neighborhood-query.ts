// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiTaskNeighborhoodQuerySchema = {"$id":"urn:kanban-tool:schema:api:task-neighborhood-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"depth":{"default":1,"format":"uint","minimum":0,"type":"integer"},"include_archived_context":{"default":false,"type":"boolean"},"limit_nodes":{"default":250,"format":"uint","minimum":0,"type":"integer"}},"title":"Kanban task neighborhood query v1","type":"object"} as const;
export type ApiTaskNeighborhoodQueryContract = FromSchema<typeof ApiTaskNeighborhoodQuerySchema>;

export const apiTaskNeighborhoodQueryValidator: ReturnType<typeof createContractValidator<ApiTaskNeighborhoodQueryContract>> = createContractValidator<ApiTaskNeighborhoodQueryContract>(
  "api.task-neighborhood.query",
  ApiTaskNeighborhoodQuerySchema,
);

export function parseApiTaskNeighborhoodQuery(value: unknown): ApiTaskNeighborhoodQueryContract {
  if (!apiTaskNeighborhoodQueryValidator(value)) throw new ContractValidationError("api.task-neighborhood.query", apiTaskNeighborhoodQueryValidator.errors);
  return value;
}
