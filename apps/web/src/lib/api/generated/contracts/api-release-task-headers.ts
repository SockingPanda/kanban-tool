// 由 `xtask web-contracts generate` 生成；请勿手工编辑。
import type { FromSchema } from "json-schema-to-ts";
import { ContractValidationError, createContractValidator } from "../runtime";
import staticValidator from "virtual:kanban-contract-validator/api-release-task-headers";

export const ApiReleaseTaskHeadersSchema = {"$id":"urn:kanban-tool:schema:api:release-task-headers:v1","$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"Accept-Language":{"type":["string","null"]},"Content-Type":{"type":"string"},"X-KB-Actor":{"type":["string","null"]}},"required":["Content-Type"],"title":"Kanban api.release-task request headers v1","type":"object"} as const;
export type ApiReleaseTaskHeadersContract = FromSchema<typeof ApiReleaseTaskHeadersSchema>;

export const apiReleaseTaskHeadersValidator: ReturnType<typeof createContractValidator<ApiReleaseTaskHeadersContract>> = createContractValidator<ApiReleaseTaskHeadersContract>(
  "api.release-task.headers",
  staticValidator,
);

export function parseApiReleaseTaskHeaders(value: unknown): ApiReleaseTaskHeadersContract {
  if (!apiReleaseTaskHeadersValidator(value)) throw new ContractValidationError("api.release-task.headers", apiReleaseTaskHeadersValidator.errors);
  return value;
}
