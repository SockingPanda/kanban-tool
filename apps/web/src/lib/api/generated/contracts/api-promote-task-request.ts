// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiPromoteTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:promote-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]}},"title":"Kanban promote task request v1","type":"object"} as const;
export type ApiPromoteTaskRequestContract = FromSchema<typeof ApiPromoteTaskRequestSchema>;

export const apiPromoteTaskRequestValidator: ReturnType<typeof createContractValidator<ApiPromoteTaskRequestContract>> = createContractValidator<ApiPromoteTaskRequestContract>(
  "api.promote-task.request",
  ApiPromoteTaskRequestSchema,
);

export function parseApiPromoteTaskRequest(value: unknown): ApiPromoteTaskRequestContract {
  if (!apiPromoteTaskRequestValidator(value)) throw new ContractValidationError("api.promote-task.request", apiPromoteTaskRequestValidator.errors);
  return value;
}
