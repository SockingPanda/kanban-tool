// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const SseStreamEventsHeadersSchema = {"$id":"urn:kanban-tool:schema:sse:stream-events-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"description":"`/api/v1/stream/events` 的 exact request headers。","properties":{"Accept-Language":{"default":null,"type":["string","null"]},"Last-Event-ID":{"default":null,"type":["string","null"]}},"title":"Kanban SSE stream events request headers v1","type":"object"} as const;
export type SseStreamEventsHeadersContract = FromSchema<typeof SseStreamEventsHeadersSchema>;

export const sseStreamEventsHeadersValidator: ReturnType<typeof createContractValidator<SseStreamEventsHeadersContract>> = createContractValidator<SseStreamEventsHeadersContract>(
  "sse.stream-events.headers",
  SseStreamEventsHeadersSchema,
);

export function parseSseStreamEventsHeaders(value: unknown): SseStreamEventsHeadersContract {
  if (!sseStreamEventsHeadersValidator(value)) throw new ContractValidationError("sse.stream-events.headers", sseStreamEventsHeadersValidator.errors);
  return value;
}
