// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiBlockTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:block-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"claim_token":{"type":["string","null"]},"force":{"default":false,"type":"boolean"},"reason":{"type":"string"}},"required":["reason"],"title":"Kanban block task request v1","type":"object"} as const;
export type ApiBlockTaskRequestContract = FromSchema<typeof ApiBlockTaskRequestSchema>;

export const apiBlockTaskRequestValidator: ReturnType<typeof createContractValidator<ApiBlockTaskRequestContract>> = createContractValidator<ApiBlockTaskRequestContract>(
  "api.block-task.request",
  ApiBlockTaskRequestSchema,
);

export function parseApiBlockTaskRequest(value: unknown): ApiBlockTaskRequestContract {
  if (!apiBlockTaskRequestValidator(value)) throw new ContractValidationError("api.block-task.request", apiBlockTaskRequestValidator.errors);
  return value;
}
