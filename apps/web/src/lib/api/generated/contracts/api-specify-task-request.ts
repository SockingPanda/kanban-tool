// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiSpecifyTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:specify-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"description":{"type":["string","null"]},"scheduled_at":{"format":"int64","type":["integer","null"]}},"title":"Kanban specify task request v1","type":"object"} as const;
export type ApiSpecifyTaskRequestContract = FromSchema<typeof ApiSpecifyTaskRequestSchema>;

export const apiSpecifyTaskRequestValidator: ReturnType<typeof createContractValidator<ApiSpecifyTaskRequestContract>> = createContractValidator<ApiSpecifyTaskRequestContract>(
  "api.specify-task.request",
  ApiSpecifyTaskRequestSchema,
);

export function parseApiSpecifyTaskRequest(value: unknown): ApiSpecifyTaskRequestContract {
  if (!apiSpecifyTaskRequestValidator(value)) throw new ContractValidationError("api.specify-task.request", apiSpecifyTaskRequestValidator.errors);
  return value;
}
