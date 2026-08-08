// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiCompleteTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:complete-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"claim_token":{"type":["string","null"]},"force":{"default":false,"type":"boolean"},"result":true,"summary":{"type":["string","null"]}},"title":"Kanban complete task request v1","type":"object"} as const;
export type ApiCompleteTaskRequestContract = FromSchema<typeof ApiCompleteTaskRequestSchema>;

export const apiCompleteTaskRequestValidator: ReturnType<typeof createContractValidator<ApiCompleteTaskRequestContract>> = createContractValidator<ApiCompleteTaskRequestContract>(
  "api.complete-task.request",
  ApiCompleteTaskRequestSchema,
);

export function parseApiCompleteTaskRequest(value: unknown): ApiCompleteTaskRequestContract {
  if (!apiCompleteTaskRequestValidator(value)) throw new ContractValidationError("api.complete-task.request", apiCompleteTaskRequestValidator.errors);
  return value;
}
