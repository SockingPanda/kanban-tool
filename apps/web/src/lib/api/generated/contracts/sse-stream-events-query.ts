// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const SseStreamEventsQuerySchema = {"$id":"urn:kanban-tool:schema:sse:stream-events-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"after":{"default":0,"format":"int64","type":"integer"},"board":{"default":"default","type":"string"},"limit":{"default":100,"format":"uint","minimum":0,"type":"integer"},"task_id":{"type":["string","null"]}},"title":"Kanban SSE stream events query v1","type":"object"} as const;
export type SseStreamEventsQueryContract = FromSchema<typeof SseStreamEventsQuerySchema>;

export const sseStreamEventsQueryValidator: ReturnType<typeof createContractValidator<SseStreamEventsQueryContract>> = createContractValidator<SseStreamEventsQueryContract>(
  "sse.stream-events.query",
  SseStreamEventsQuerySchema,
);

export function parseSseStreamEventsQuery(value: unknown): SseStreamEventsQueryContract {
  if (!sseStreamEventsQueryValidator(value)) throw new ContractValidationError("sse.stream-events.query", sseStreamEventsQueryValidator.errors);
  return value;
}
