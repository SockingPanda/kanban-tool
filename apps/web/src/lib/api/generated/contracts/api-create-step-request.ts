// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCreateStepRequestSchema = {"$id":"urn:kanban-tool:schema:api:create-step-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"body":{"type":["string","null"]},"idempotency_key":{"type":["string","null"]},"linked_task_ref":{"type":["string","null"]},"position":{"format":"int64","type":["integer","null"]},"required":{"default":true,"type":"boolean"},"title":{"type":"string"}},"required":["title"],"title":"Kanban create step request v1","type":"object"} as const;
export type ApiCreateStepRequestContract = FromSchema<typeof ApiCreateStepRequestSchema>;

export const apiCreateStepRequestValidator: ReturnType<typeof createContractValidator<ApiCreateStepRequestContract>> = createContractValidator<ApiCreateStepRequestContract>(
  "api.create-step.request",
  ApiCreateStepRequestSchema,
);

export function parseApiCreateStepRequest(value: unknown): ApiCreateStepRequestContract {
  if (!apiCreateStepRequestValidator(value)) throw new ContractValidationError("api.create-step.request", apiCreateStepRequestValidator.errors);
  return value;
}
