// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";

export const ApiArchiveTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:archive-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":["string","null"]},"X-KB-Actor":{"type":["string","null"]}},"title":"Kanban api.archive-task request headers v1","type":"object"} as const;
export type ApiArchiveTaskHeadersContract = FromSchema<typeof ApiArchiveTaskHeadersSchema>;

export const apiArchiveTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiArchiveTaskHeadersContract>> = createContractValidator<ApiArchiveTaskHeadersContract>(
  "api.archive-task.headers",
  ApiArchiveTaskHeadersSchema,
);

export function parseApiArchiveTaskHeaders(value: unknown): ApiArchiveTaskHeadersContract {
  if (!apiArchiveTaskHeadersValidator(value)) throw new ContractValidationError("api.archive-task.headers", apiArchiveTaskHeadersValidator.errors);
  return value;
}
