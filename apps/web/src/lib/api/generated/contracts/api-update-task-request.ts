// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiUpdateTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:update-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"assignee":{"type":["string","null"]},"description":{"type":["string","null"]},"due_at":{"format":"int64","type":["integer","null"]},"expected_lock_version":{"format":"int64","type":"integer"},"max_retries":{"format":"int64","type":["integer","null"]},"metadata":true,"priority":{"format":"int64","type":"integer"},"scheduled_at":{"format":"int64","type":["integer","null"]},"title":{"type":"string"}},"title":"Kanban update task request v1","type":"object"} as const;
export type ApiUpdateTaskRequestContract = FromSchema<typeof ApiUpdateTaskRequestSchema>;

export const apiUpdateTaskRequestValidator: ReturnType<typeof createContractValidator<ApiUpdateTaskRequestContract>> = createContractValidator<ApiUpdateTaskRequestContract>(
  "api.update-task.request",
  ApiUpdateTaskRequestSchema,
);

export function parseApiUpdateTaskRequest(value: unknown): ApiUpdateTaskRequestContract {
  if (!apiUpdateTaskRequestValidator(value)) throw new ContractValidationError("api.update-task.request", apiUpdateTaskRequestValidator.errors);
  return value;
}
