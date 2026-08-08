// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiAddTaskLabelHeadersSchema = {"$id":"urn:kanban-tool:schema:api:add-task-label-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.add-task-label request headers v1","type":"object"} as const;
export type ApiAddTaskLabelHeadersContract = FromSchema<typeof ApiAddTaskLabelHeadersSchema>;

export const apiAddTaskLabelHeadersValidator: ReturnType<typeof createContractValidator<ApiAddTaskLabelHeadersContract>> = createContractValidator<ApiAddTaskLabelHeadersContract>(
  "api.add-task-label.headers",
  ApiAddTaskLabelHeadersSchema,
);

export function parseApiAddTaskLabelHeaders(value: unknown): ApiAddTaskLabelHeadersContract {
  if (!apiAddTaskLabelHeadersValidator(value)) throw new ContractValidationError("api.add-task-label.headers", apiAddTaskLabelHeadersValidator.errors);
  return value;
}
