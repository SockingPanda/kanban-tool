// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiListTaskLabelsResponseSchema = {"$defs":{"ApiLabel":{"additionalProperties":false,"properties":{"board_id":{"type":"string"},"color":{"type":["string","null"]},"created_at":{"format":"int64","type":"integer"},"id":{"type":"string"},"name":{"type":"string"},"updated_at":{"format":"int64","type":"integer"}},"required":["id","board_id","name","color","created_at","updated_at"],"type":"object"}},"$id":"urn:kanban-tool:schema:api:list-task-labels-response:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"data":{"items":{"$ref":"#/$defs/ApiLabel"},"type":"array"}},"required":["data"],"title":"Kanban list task labels response v1","type":"object"} as const;
export type ApiListTaskLabelsResponseContract = FromSchema<typeof ApiListTaskLabelsResponseSchema>;

export const apiListTaskLabelsResponseValidator: ReturnType<typeof createContractValidator<ApiListTaskLabelsResponseContract>> = createContractValidator<ApiListTaskLabelsResponseContract>(
  "api.list-task-labels.response",
  ApiListTaskLabelsResponseSchema,
);

export function parseApiListTaskLabelsResponse(value: unknown): ApiListTaskLabelsResponseContract {
  if (!apiListTaskLabelsResponseValidator(value)) throw new ContractValidationError("api.list-task-labels.response", apiListTaskLabelsResponseValidator.errors);
  return value;
}
