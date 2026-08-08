// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiTaskNeighborhoodHeadersSchema = {"$id":"urn:kanban-tool:schema:api:task-neighborhood-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]}},"title":"Kanban api.task-neighborhood request headers v1","type":"object"} as const;
export type ApiTaskNeighborhoodHeadersContract = FromSchema<typeof ApiTaskNeighborhoodHeadersSchema>;

export const apiTaskNeighborhoodHeadersValidator: ReturnType<typeof createContractValidator<ApiTaskNeighborhoodHeadersContract>> = createContractValidator<ApiTaskNeighborhoodHeadersContract>(
  "api.task-neighborhood.headers",
  ApiTaskNeighborhoodHeadersSchema,
);

export function parseApiTaskNeighborhoodHeaders(value: unknown): ApiTaskNeighborhoodHeadersContract {
  if (!apiTaskNeighborhoodHeadersValidator(value)) throw new ContractValidationError("api.task-neighborhood.headers", apiTaskNeighborhoodHeadersValidator.errors);
  return value;
}
