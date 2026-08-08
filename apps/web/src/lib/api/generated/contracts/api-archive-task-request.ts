// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiArchiveTaskRequestSchema = {"$id":"urn:kanban-tool:schema:api:archive-task-request:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actor":{"type":["string","null"]},"force":{"default":false,"type":"boolean"}},"title":"Kanban archive task request v1","type":"object"} as const;
export type ApiArchiveTaskRequestContract = FromSchema<typeof ApiArchiveTaskRequestSchema>;

export const apiArchiveTaskRequestValidator: ReturnType<typeof createContractValidator<ApiArchiveTaskRequestContract>> = createContractValidator<ApiArchiveTaskRequestContract>(
  "api.archive-task.request",
  ApiArchiveTaskRequestSchema,
);

export function parseApiArchiveTaskRequest(value: unknown): ApiArchiveTaskRequestContract {
  if (!apiArchiveTaskRequestValidator(value)) throw new ContractValidationError("api.archive-task.request", apiArchiveTaskRequestValidator.errors);
  return value;
}
