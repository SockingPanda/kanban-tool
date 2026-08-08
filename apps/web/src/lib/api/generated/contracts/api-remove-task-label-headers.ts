// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiRemoveTaskLabelHeadersSchema = {"$id":"urn:kanban-tool:schema:api:remove-task-label-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.remove-task-label request headers v1","type":"object"} as const;
export type ApiRemoveTaskLabelHeadersContract = FromSchema<typeof ApiRemoveTaskLabelHeadersSchema>;

export const apiRemoveTaskLabelHeadersValidator: ReturnType<typeof createContractValidator<ApiRemoveTaskLabelHeadersContract>> = createContractValidator<ApiRemoveTaskLabelHeadersContract>(
  "api.remove-task-label.headers",
  ApiRemoveTaskLabelHeadersSchema,
);

export function parseApiRemoveTaskLabelHeaders(value: unknown): ApiRemoveTaskLabelHeadersContract {
  if (!apiRemoveTaskLabelHeadersValidator(value)) throw new ContractValidationError("api.remove-task-label.headers", apiRemoveTaskLabelHeadersValidator.errors);
  return value;
}
