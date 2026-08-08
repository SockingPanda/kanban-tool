// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiBoardTaskMapQuerySchema = {"$id":"urn:kanban-tool:schema:api:board-task-map-query:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"active_only":{"default":true,"type":"boolean"},"context_depth":{"default":1,"format":"uint","minimum":0,"type":"integer"},"hide_isolated":{"default":false,"type":"boolean"},"include_archived_context":{"default":false,"type":"boolean"},"include_done_context":{"default":true,"type":"boolean"},"limit_nodes":{"default":250,"format":"uint","minimum":0,"type":"integer"}},"title":"Kanban board task map query v1","type":"object"} as const;
export type ApiBoardTaskMapQueryContract = FromSchema<typeof ApiBoardTaskMapQuerySchema>;

export const apiBoardTaskMapQueryValidator: ReturnType<typeof createContractValidator<ApiBoardTaskMapQueryContract>> = createContractValidator<ApiBoardTaskMapQueryContract>(
  "api.board-task-map.query",
  ApiBoardTaskMapQuerySchema,
);

export function parseApiBoardTaskMapQuery(value: unknown): ApiBoardTaskMapQueryContract {
  if (!apiBoardTaskMapQueryValidator(value)) throw new ContractValidationError("api.board-task-map.query", apiBoardTaskMapQueryValidator.errors);
  return value;
}
