// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const SseEventHeartbeatSchema = {"$id":"urn:kanban-tool:schema:sse:event-heartbeat:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"description":"连接保活 frame 的 typed data。它没有业务 cursor 或 envelope 字段。","title":"Kanban SSE transport heartbeat v1","type":"object"} as const;
export type SseEventHeartbeatContract = FromSchema<typeof SseEventHeartbeatSchema>;

export const sseEventHeartbeatValidator: ReturnType<typeof createContractValidator<SseEventHeartbeatContract>> = createContractValidator<SseEventHeartbeatContract>(
  "sse.event.heartbeat",
  SseEventHeartbeatSchema,
);

export function parseSseEventHeartbeat(value: unknown): SseEventHeartbeatContract {
  if (!sseEventHeartbeatValidator(value)) throw new ContractValidationError("sse.event.heartbeat", sseEventHeartbeatValidator.errors);
  return value;
}
