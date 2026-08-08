// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCheckpointResponseSchema = {"$defs":{"CheckpointReport":{"additionalProperties":false,"properties":{"busy":{"format":"int64","type":"integer"},"checkpointed_frames":{"format":"int64","type":"integer"},"log_frames":{"format":"int64","type":"integer"}},"required":["busy","log_frames","checkpointed_frames"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:checkpoint-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"$ref":"#/$defs/CheckpointReport"}},"required":["data"],"title":"Kanban checkpoint response v1","type":"object"} as const;
export type ApiCheckpointResponseContract = FromSchema<typeof ApiCheckpointResponseSchema>;

export const apiCheckpointResponseValidator: ReturnType<typeof createContractValidator<ApiCheckpointResponseContract>> = createContractValidator<ApiCheckpointResponseContract>(
  "api.checkpoint.response",
  ApiCheckpointResponseSchema,
);

export function parseApiCheckpointResponse(value: unknown): ApiCheckpointResponseContract {
  if (!apiCheckpointResponseValidator(value)) throw new ContractValidationError("api.checkpoint.response", apiCheckpointResponseValidator.errors);
  return value;
}
