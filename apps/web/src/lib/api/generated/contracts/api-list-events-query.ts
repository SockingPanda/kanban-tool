// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListEventsQuerySchema = {"$id":"urn:kanban-tool:schema:api:list-events-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"after":{"default":0,"format":"int64","type":"integer"},"board":{"default":"default","type":"string"},"limit":{"default":100,"format":"uint","minimum":0,"type":"integer"},"task_id":{"type":["string","null"]}},"title":"Kanban list events query v1","type":"object"} as const;
export type ApiListEventsQueryContract = FromSchema<typeof ApiListEventsQuerySchema>;

export const apiListEventsQueryValidator: ReturnType<typeof createContractValidator<ApiListEventsQueryContract>> = createContractValidator<ApiListEventsQueryContract>(
  "api.list-events.query",
  ApiListEventsQuerySchema,
);

export function parseApiListEventsQuery(value: unknown): ApiListEventsQueryContract {
  if (!apiListEventsQueryValidator(value)) throw new ContractValidationError("api.list-events.query", apiListEventsQueryValidator.errors);
  return value;
}
