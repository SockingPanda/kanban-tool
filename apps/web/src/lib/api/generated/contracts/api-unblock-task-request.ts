// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-unblock-task-request";

export const ApiUnblockTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:unblock-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]}},"title":"Kanban unblock task request v1","type":"object"} as const;
export type ApiUnblockTaskRequestContract = FromSchema<typeof ApiUnblockTaskRequestSchema>;

export const apiUnblockTaskRequestValidator: ReturnType<typeof createContractValidator<ApiUnblockTaskRequestContract>> = createContractValidator<ApiUnblockTaskRequestContract>(
  "api.unblock-task.request",
  staticValidator,
);

export function parseApiUnblockTaskRequest(value: unknown): ApiUnblockTaskRequestContract {
  if (!apiUnblockTaskRequestValidator(value)) throw new ContractValidationError("api.unblock-task.request", apiUnblockTaskRequestValidator.errors);
  return value;
}
