// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiTaskNeighborhoodPathSchema = {"$id":"urn:kanban-tool:schema:api:task-neighborhood-path:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"task_id":{"type":"string"}},"required":["task_id"],"title":"Kanban task neighborhood path v1","type":"object"} as const;
export type ApiTaskNeighborhoodPathContract = FromSchema<typeof ApiTaskNeighborhoodPathSchema>;

export const apiTaskNeighborhoodPathValidator: ReturnType<typeof createContractValidator<ApiTaskNeighborhoodPathContract>> = createContractValidator<ApiTaskNeighborhoodPathContract>(
  "api.task-neighborhood.path",
  ApiTaskNeighborhoodPathSchema,
);

export function parseApiTaskNeighborhoodPath(value: unknown): ApiTaskNeighborhoodPathContract {
  if (!apiTaskNeighborhoodPathValidator(value)) throw new ContractValidationError("api.task-neighborhood.path", apiTaskNeighborhoodPathValidator.errors);
  return value;
}
